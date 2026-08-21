# RAH v0.2 Tool Bridge Research

Date: 2026-08-21

Status: Architecture spike only. This document does not implement the production
Tool Bridge or the echo prototype.

## Recommendation

**B. PROCEED WITH INTERNAL ADAPTER CHANGES.**

Codex app-server 0.148.0 dynamic tools can be adapted to the current RAH tool
boundary without changing any architecture-defining RAH public contract:

```text
Codex model
 -> item/tool/call server request
 -> private rah-runtime-codex protocol adapter
 -> RAH ToolCall / ToolInput
 -> RAH-owned permission decision
 -> ToolRegistry
 -> Tool (and Sandbox where the tool uses it)
 -> RAH ToolOutput or ToolError
 -> private DynamicToolCallResponse translation
 -> Codex model continuation
```

The current implementation is not ready to do this. `rah-runtime-codex` initializes
with `experimentalApi: false`, does not receive server-request parameters, rejects
every server request, has no `ToolRegistry`, and treats dynamic-tool item
notifications as unsupported Codex-owned activity. Those are adapter-local gaps,
not defects in `AgentRuntime`, `Tool`, `ToolDefinition`, `ToolCall`, `ToolInput`,
`ToolOutput`, `ToolRegistry`, `Sandbox`, or `AgentEvent`.

The echo-only prototype can be implemented entirely behind the adapter boundary.
A later production integration will also need an adapter-local construction or
configuration entry point through which the host supplies the registry and its
allowed permission levels. That API must not move Codex types into a RAH-owned
contract.

## Scope and evidence baseline

This research used:

- the current RAH repository, including the accepted ADRs and the implementation
  present on 2026-08-21;
- local `codex-cli 0.148.0`;
- upstream tag `rust-v0.148.0`, peeled commit
  `3ba0f711642a888aec92a611a3f3b2211157ff89`;
- stable and experimental schemas generated locally with:

  ```text
  codex app-server generate-json-schema --out <dir>
  codex app-server generate-json-schema --experimental --out <dir>
  ```

The exact-version primary sources are the [0.148.0 app-server
README](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/app-server/README.md),
[thread protocol](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/app-server-protocol/src/protocol/v2/thread.rs),
[item protocol](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/app-server-protocol/src/protocol/v2/item.rs),
[dynamic-tool protocol](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/protocol/src/dynamic_tools.rs),
[app-server response adapter](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/app-server/src/dynamic_tools.rs),
[dynamic tool handler](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/core/src/tools/handlers/dynamic.rs),
and [0.148.0 integration tests](https://github.com/openai/codex/blob/3ba0f711642a888aec92a611a3f3b2211157ff89/codex-rs/app-server/tests/suite/v2/dynamic_tools.rs).

An important schema-generation detail is that stable-only output contains the
shared dynamic call request/response definitions because the server request union
uses them, but omits `thread/start.dynamicTools`. The `dynamicTools` property is
present only in output generated with `--experimental`.

## Security decision

The bridge is acceptable only with all of these invariants enforced:

1. Codex advertises and reasons about RAH-provided tool descriptions, but never
   executes a RAH tool.
2. `item/tool/call` is treated as an untrusted request, not authorization.
3. The adapter resolves the request through the RAH `ToolRegistry`.
4. Permission is checked from the registered RAH `ToolDefinition` immediately
   before execution. Codex-supplied data cannot select or lower permission.
5. A tool that uses `Sandbox` continues to use its configured RAH sandbox. The
   dynamic bridge does not replace, wrap around, or bypass it.
6. Codex-owned shell, file-change, MCP, web-search, image-view, and app tools remain
   disabled. Their items remain unsupported and fail closed.
7. Codex approval requests are never accepted automatically. They remain denied as
   unsupported server requests.
8. Only a successfully routed `item/tool/call` becomes a RAH `ToolCall`. Codex
   `item/started` and `item/completed` notifications are not translated into fake
   RAH tool lifecycle events.

Codex's `approvalPolicy: "never"` and read-only sandbox configuration remain useful
defense in depth, but neither is the RAH authorization boundary.

## Exact Codex 0.148.0 contract

### Initialization capability

The client must set the capability in its one `initialize` request:

```json
{
  "method": "initialize",
  "id": 1,
  "params": {
    "clientInfo": {
      "name": "rah-runtime-codex",
      "version": "<RAH adapter version>"
    },
    "capabilities": {
      "experimentalApi": true,
      "requestAttestation": false
    }
  }
}
```

The client then sends `initialized`. If `capabilities` is omitted,
`experimentalApi` defaults to `false`. Initialization is one-shot; attempting to
initialize again is rejected. Without opt-in, use of `dynamicTools` is rejected
with `thread/start.dynamicTools requires experimentalApi capability`.

The capability is connection-scoped in 0.148.0. Upstream source contains a TODO
noting awkward cross-client behavior when clients attached to one thread differ in
experimental capability.

### `thread/start.dynamicTools`

`dynamicTools` is an optional, nullable array on `ThreadStartParams`, defaulting to
`null`. Each array entry is one of these tagged objects.

Top-level function:

```json
{
  "type": "function",
  "name": "echo",
  "description": "Returns the supplied text unchanged.",
  "inputSchema": {
    "type": "object",
    "properties": { "text": { "type": "string" } },
    "required": ["text"],
    "additionalProperties": false
  },
  "deferLoading": false
}
```

Namespace:

```json
{
  "type": "namespace",
  "name": "example_namespace",
  "description": "Example tools.",
  "tools": [
    {
      "type": "function",
      "name": "example_tool",
      "description": "Example tool.",
      "inputSchema": {},
      "deferLoading": false
    }
  ]
}
```

For function specs, `type`, `name`, `description`, and `inputSchema` are required;
`deferLoading` is optional and defaults to `false`. For namespace specs, `type`,
`name`, `description`, and `tools` are required. Namespace entries are function
specs with the same fields.

Codex 0.148.0 applies these validations before starting the thread:

- function names are 1 through 128 characters and match
  `^[a-zA-Z0-9_-]+$`;
- namespace names are 1 through 64 characters and use the same character set;
- leading or trailing whitespace is rejected;
- duplicate top-level function names, duplicate namespaces, and duplicate
  function names within a namespace are rejected;
- names `mcp` and names beginning `mcp__` are reserved;
- namespace names also cannot collide with reserved Responses namespaces;
- namespaces must contain at least one tool;
- namespace descriptions are limited to 1,024 characters;
- deferred tools must be inside a namespace; and
- each `inputSchema` must be accepted by Codex's Responses tool-schema parser.

No description-length limit for a function is declared in this validation path.

### `item/tool/call` request

When the model calls a dynamic tool, app-server emits `item/started` for a
`dynamicToolCall` and then sends this server-to-client JSON-RPC request:

```json
{
  "method": "item/tool/call",
  "id": 60,
  "params": {
    "threadId": "thr_123",
    "turnId": "turn_123",
    "callId": "call_123",
    "namespace": null,
    "tool": "echo",
    "arguments": { "text": "hello" }
  }
}
```

The JSON-RPC `id` is a string or signed 64-bit integer and correlates the client
response. It is distinct from `params.callId`, which identifies the logical model
tool call and the surrounding dynamic-tool item.

Exact `DynamicToolCallParams` fields are:

| Field | Wire type | Required | Meaning |
| --- | --- | --- | --- |
| `threadId` | string | yes | Codex thread owner |
| `turnId` | string | yes | Active Codex turn owner |
| `callId` | string | yes | Logical tool call and dynamic item ID |
| `namespace` | string or null | no | Namespace, if a namespace tool was exposed |
| `tool` | string | yes | Function name, not including namespace |
| `arguments` | any JSON value | yes | Untrusted model-supplied arguments |

The generated schema intentionally places no structural restriction on
`arguments`; the advertised tool schema and the RAH tool implementation provide
the semantic contract.

### `DynamicToolCallResponse`

The client responds using the same JSON-RPC request `id`:

```json
{
  "id": 60,
  "result": {
    "contentItems": [
      { "type": "inputText", "text": "hello" }
    ],
    "success": true
  }
}
```

Both `contentItems` and `success` are required. `contentItems` is an array of:

| Variant | Required value |
| --- | --- |
| `inputText` | string `text` |
| `inputImage` | string `imageUrl` |
| `inputAudio` | string `audioUrl` |

Remote HTTP(S) image URLs are rejected by the 0.148.0 app-server response adapter.
Audio URLs must be inline `data:` URLs. Invalid response shape or invalid media
URL is converted upstream into `success: false` with a text fallback. RAH v0.1
has only text and JSON output content, so the proposed bridge needs only
`inputText`.

After a valid response, Codex submits the output into the active core thread,
emits `item/completed` with `status: "completed"` or `"failed"`, and makes the
content available to the next model continuation.

## RAH-to-Codex mappings

### `ToolDefinition` to `DynamicToolSpec`

Use a top-level function spec for v0.2. Namespaces and deferred loading add no
value to the initial bridge.

| RAH field | Codex field | Rule |
| --- | --- | --- |
| `name` | `name` | Use the exact name only when it satisfies Codex naming rules; otherwise allocate a collision-free thread-local alias and retain a private alias-to-`ToolName` map. |
| `description` | `description` | Copy unchanged. |
| `input_schema` | `inputSchema` | Copy the JSON value unchanged, then let `thread/start` reject schemas unsupported by Codex. Prefer preflight validation against captured compatibility fixtures for deterministic errors. |
| `permission` | none | Never expose it to Codex. Retain and enforce it in RAH immediately before dispatch. |
| none | `type` | Set to `"function"`. |
| none | `deferLoading` | Omit or set `false` for v0.2. |

RAH intentionally imposes no transport-specific restrictions on `ToolName`.
Current built-ins such as `fs.read` and `shell.exec` contain dots, which Codex
rejects. This is an adapter translation issue, not a reason to change
`ToolName`. A private alias table also prevents Codex from using an arbitrary
string to address a registry entry.

At thread start, snapshot the registry definitions, validate/alias them, and bind
that exact set to the private RAH-session/Codex-thread record. Since
`ToolRegistry::register` requires mutable access and runtime code normally holds
it behind `Arc`, the registry is naturally frozen during use.

### `DynamicToolCallParams` to `ToolCall` / `ToolInput`

Before constructing a `ToolCall`, validate all routing fields against adapter-owned
state:

1. `threadId` must map to exactly one current RAH `SessionId`.
2. `turnId` must equal that session's active turn.
3. `(namespace, tool)` must resolve through that thread's advertised alias table.
   The initial bridge requires `namespace: null`.
4. The resolved RAH name must still exist in `ToolRegistry`.
5. `callId` must pass duplicate/replay checks described below.

Then map:

```text
ToolCall {
    id: newly generated RAH ToolCallId,
    name: privately resolved RAH ToolName,
    input: ToolInput(arguments),
}
```

`ToolCallId` is a RAH-owned UUID and cannot, and should not, expose a Codex string
ID. Keep a private mapping from `(threadId, turnId, callId)` to `ToolCallId` and
the JSON-RPC request IDs waiting for its result.

### `ToolOutput` / `ToolError` to `DynamicToolCallResponse`

Map ordered RAH content as follows:

- `ToolContent::Text(text)` becomes `{ "type": "inputText", "text": text }`.
- `ToolContent::Json(value)` becomes one `inputText` item containing compact JSON
  produced by `serde_json`. Codex 0.148.0 has no arbitrary JSON output item.
- `ToolOutput.is_error == false` becomes `success: true`.
- `ToolOutput.is_error == true` becomes `success: false`.

For `ToolError`, produce a bounded, sanitized `inputText` error and
`success: false`. Preserve the structured RAH error for diagnostics and RAH event
handling; do not expose sensitive internal detail merely because Codex will show
the text to the model.

An empty RAH content vector maps to an empty `contentItems` array, which is valid
under the 0.148.0 schema.

## Current RAH execution boundary

### `ToolRegistry`

The existing `ToolRegistry` can execute bridged calls without modification. It:

- owns a provider-neutral `ToolName -> Arc<dyn Tool>` map;
- rejects duplicate registration;
- exposes deterministically sorted `ToolDefinition` values;
- fails unknown lookup with `ToolError::UnknownTool`; and
- dispatches a `ToolCall` with `ToolContext` to the registered `Tool`.

The registry deliberately does not authorize. The caller must check permission
before calling `ToolRegistry::execute`.

`rah-runtime-codex` does not currently depend on `rah-tools`. A prototype would
add the adapter-local dependency edge `rah-runtime-codex -> rah-tools` because the
adapter must own or reference a registry. This respects the dependency-bottom
position of `rah-protocol` and does not add any provider dependency to core.

### Permission and policy

The only executable permission check in the current repository is in
`MinimalTestRuntime`. It looks up the registered tool, reads
`tool.definition().permission`, compares it with the runtime's explicit
`allowed_permissions`, and only then calls `ToolRegistry::execute`. The default
allow-list contains only `PermissionLevel::None`; additional levels are opt-in.

`CodexRuntime` currently has neither a registry nor an allowed-permissions policy.
The bridge must add adapter-owned equivalents and use the same order:

```text
untrusted request
 -> alias and registry lookup
 -> permission from registered ToolDefinition
 -> host-configured allow decision
 -> ToolRegistry::execute
```

Never trust a cached or Codex-supplied permission. Re-read the definition from the
registered tool at dispatch time and fail closed on a mismatch with the advertised
snapshot.

No new approval-response contract is required for the echo prototype because
`echo` requires `PermissionLevel::None`. Higher-permission tools must remain
disabled until the adapter has an explicit host configuration; Codex approval
requests are unrelated and remain unsupported.

### Sandbox authority

`Sandbox` remains authoritative for applicable tools because sandbox use is an
implementation detail of the RAH `Tool`:

- `ShellExecTool` always calls its configured `Sandbox::execute` with a direct
  program/argument vector and its configured `SandboxPolicy`.
- `FsReadTool` enforces its configured `WorkspacePolicy` directly.
- `EchoTool` requires neither filesystem nor process authority.

The bridge calls only `ToolRegistry::execute`; it receives no capability to invoke
subprocesses or access files independently. Therefore it cannot bypass a tool's
sandbox/workspace boundary. Codex's own shell, file, and MCP mechanisms remain
disabled and are not substitutes for RAH `Sandbox`.

## Required lifecycle behavior

### Cancellation while a call is pending

Codex waits for the `item/tool/call` response before model continuation. RAH must
not let that wait detach tool execution from session cancellation.

The adapter should keep each pending call owned by the session/turn and race tool
execution against the same turn cancellation state used by `AgentRuntime::cancel`
and stream-drop cleanup. On cancellation:

1. atomically mark the call cancelled so a late result cannot be returned;
2. drop/cancel the RAH tool future, matching `MinimalTestRuntime` behavior;
3. send `turn/interrupt` for the exact active thread and turn;
4. do not wait for a dynamic `item/completed`; and
5. use terminal `turn/completed(status = "interrupted")` as the Codex-side
   cancellation confirmation.

Cancellation is cooperative at Rust future boundaries and cannot roll back a side
effect that already happened. Tools with external side effects still require
their own timeout/idempotency design.

An open upstream report, [issue
#33993](https://github.com/openai/codex/issues/33993), demonstrates that a pending
dynamic tool may emit `item/started` but no matching `item/completed` when the turn
is interrupted. The 0.148.0 handler still awaits the client response between those
events. RAH must therefore reconcile pending calls at the turn level rather than
depending on perfect item lifecycle symmetry.

### Duplicate and replayed calls

Treat `(threadId, turnId, callId)` as the logical Codex call key. The JSON-RPC
request `id` is transport correlation only.

Maintain a bounded per-session table with these states:

- `InFlight { rah_tool_call_id, canonical_tool, canonical_arguments, waiters }`;
- `Completed { canonical_tool, canonical_arguments, response }`;
- `Cancelled`.

Rules:

- First sighting allocates one new `ToolCallId` and executes once.
- A duplicate with identical tool identity and arguments joins the in-flight call
  or receives the cached response; it never executes again.
- Reuse of a key with different tool identity or arguments is a protocol
  violation and fails closed.
- A call from a non-active turn, unknown thread, or cancelled state is rejected
  without execution.
- Clear or age out completed entries only after the turn reaches a terminal state;
  keep a bounded tombstone long enough to reject late duplicates.

This state is private because Codex call IDs are transport/runtime details. The
RAH `ToolCallId` contract does not need parsing or provider-specific variants.

### Thread/session ownership and routing

The current adapter already privately maps each RAH `SessionId` to one Codex
`threadId` and active `turnId`. Dynamic dispatch must be centralized at the
connection/session layer, not broadcast blindly to every event stream. A server
request must carry its JSON-RPC `id`, method, and params to exactly one routed
handler with a one-shot response authority.

For v0.2, preserve a single stdio app-server connection as the sole dynamic-tool
responder. Do not attach another client connection to a tool-bearing thread. An
open upstream report, [issue
#35894](https://github.com/openai/codex/issues/35894), shows that app-server can
broadcast one dynamic-tool request to all subscribers and accept the first
response. Inspection of 0.148.0 still shows thread-scoped requests sent to the
subscriber list and pending callbacks keyed by request ID rather than responder
connection. The single-owner restriction prevents that race from crossing the
RAH boundary.

### Unknown tools

An unknown alias or a registry miss must never execute. Return a sanitized
`success: false` response if the turn is still active so Codex's pending request is
released, record a RAH tool/protocol failure, and apply the runtime's chosen
terminal policy. For the first prototype, fail the RAH operation and interrupt
the Codex turn rather than allowing an unadvertised capability request to proceed.

### Malformed arguments

There are two cases:

- A malformed JSON-RPC request or missing/wrongly typed required field is a
  protocol error. Respond with a JSON-RPC invalid-params error and never construct
  a `ToolCall`.
- Syntactically valid JSON that violates the RAH tool's semantic input contract is
  untrusted `ToolInput`. Let the registered tool validate it. Convert
  `ToolError::InvalidInput` to a sanitized `success: false` result. For the initial
  prototype, then fail/interrupt the RAH turn consistently with the current
  runtime's handling of `ToolError`.

Codex core parses model-emitted function arguments before sending
`item/tool/call`; invalid raw argument JSON can therefore fail inside Codex before
RAH receives a request. RAH must still validate every request it does receive.

### Tool failure

If a tool returns `Ok(ToolOutput { is_error: true, .. })`, respond with
`success: false`; this is a normal tool result that Codex may use for continuation.
If it returns `Err(ToolError)`, release the pending app-server request with a
sanitized failure response, retain the typed error locally, and terminate the RAH
operation consistently with current `MinimalTestRuntime` semantics. Do not report
a RAH `ToolFinished` event unless RAH actually dispatched and observed a completed
tool result.

Permission denial occurs before `ToolStarted` and before registry execution. It
must not be disguised as successful execution.

### App-server disconnect during execution

For the stdio architecture, disconnect means the process/transport needed for
Codex continuation is gone. The adapter should:

1. atomically mark every pending call for that connection failed;
2. cancel/drop their RAH execution futures where still possible;
3. emit one terminal RAH runtime failure per affected active session;
4. discard late tool results; and
5. never auto-replay the call after reconnect.

If a side effect completed just before disconnect, its outcome is uncertain from
Codex's perspective. Automatic retry could duplicate it. Recovery therefore
requires explicit user/host reconciliation, not transparent replay.

### Immutability after `thread/start`

In 0.148.0, dynamic tools are supplied only by `thread/start`. They are absent
from `thread/settings/update`, `turn/start`, and `thread/resume`; there is no method
to add, remove, or replace them after thread creation. [Upstream issue
#24808](https://github.com/openai/codex/issues/24808) requests such an API and
confirms the current limitation.

Consequences for RAH:

- the tool-definition/alias snapshot is thread-scoped and immutable;
- registry changes require a new Codex thread for the initial bridge;
- do not advertise tools whose authorization may disappear without terminating
  the thread; and
- process-restart resume must fail closed unless exact dynamic-tool restoration
  is proven. The 0.148.0 resume path does not accept a replacement dynamic-tool
  list when reconstructing a thread.

## Experimental API risks

1. **No stability guarantee.** Names, fields, gating, and lifecycle behavior can
   change between Codex versions. RAH's exact executable pin and captured schema
   verification are mandatory.
2. **Broader experimental surface.** Setting `experimentalApi: true` opts the
   connection into experimental methods and fields beyond dynamic tools. Unknown
   requests must still fail closed; unknown notifications must remain additive
   unless they indicate prohibited action.
3. **Schema split.** Stable-only generated schema omits the field needed to start
   the bridge. Compatibility validation must explicitly generate/capture the
   experimental schema.
4. **Connection-scoped capability.** Shared-thread clients can disagree about
   experimental support. RAH must remain the sole subscriber/responder.
5. **Cancellation lifecycle gap.** A terminal turn may arrive without a terminal
   dynamic item.
6. **No dynamic registry update.** Long-lived threads cannot safely track a
   changing host registry.
7. **Limited output types.** RAH JSON output must be encoded as text; native
   structured JSON output is unavailable.
8. **Name/schema restrictions.** Valid RAH definitions may be invalid Codex
   definitions. Alias and preflight logic are required.
9. **Weak effect provenance.** `DynamicToolCallParams` carries thread, turn, and
   call identity but not the originating client input. [Upstream issue
   #36994](https://github.com/openai/codex/issues/36994) describes the resulting
   limitation for per-input authorization, especially with `turn/steer`. The
   initial RAH bridge should not support steer-dependent per-input policy.

## Known upstream issues relevant to the bridge

| Issue | Relevance and mitigation |
| --- | --- |
| [#24808: update dynamicTools after thread/start](https://github.com/openai/codex/issues/24808) | Registry is immutable. Snapshot tools per thread and recreate the thread for changes. |
| [#33993: missing item/completed on interrupt](https://github.com/openai/codex/issues/33993) | Reconcile cancellation by terminal turn status, not dynamic item completion. |
| [#35894: request broadcast and first response wins](https://github.com/openai/codex/issues/35894) | Keep one RAH-owned app-server connection/subscriber for tool-bearing threads. Never allow another client to race as responder. |
| [#36994: missing originating input ID](https://github.com/openai/codex/issues/36994) | Do not claim per-input provenance or build policy that requires it. Thread/turn/call routing remains sufficient for the echo prototype. |
| [#32659: stream stall with empty custom-tool input](https://github.com/openai/codex/issues/32659) | Reported on 0.144.1 and not proven for 0.148.0, but it justifies a bounded pending-call watchdog and deterministic turn cancellation. |

These are upstream reports, not all maintainer-confirmed specifications. The first
four align with source-level properties observed in or still relevant to 0.148.0.

## Minimal echo prototype evaluation

The proposed prototype is feasible without production bridge code or public RAH
contract changes.

### Setup

- Construct a `ToolRegistry` containing only `EchoTool`.
- Allow only `PermissionLevel::None`.
- Initialize app-server with `experimentalApi: true`.
- Preserve the existing restrictions that disable Codex shell, unified execution,
  file changes, MCP servers, web search, image view, and apps.
- Add exactly one top-level `echo` function from `EchoTool::definition()` to
  `thread/start.dynamicTools`.

### Expected successful flow

```text
1. Codex reasons that echo is needed.
2. Codex emits item/started(dynamicToolCall).
3. app-server requests item/tool/call with tool=echo.
4. rah-runtime-codex validates thread, turn, call ID, alias, and arguments shape.
5. rah-runtime-codex creates a RAH ToolCall and emits ToolRequested.
6. The registered echo definition requires PermissionLevel::None, which is allowed.
7. rah-runtime-codex emits ToolStarted and calls ToolRegistry::execute.
8. EchoTool validates {"text": <string>} and returns ToolOutput::Text.
9. rah-runtime-codex emits ToolFinished from that actual RAH result.
10. The adapter responds with contentItems=[inputText] and success=true.
11. Codex emits its own item/completed notification; the adapter does not turn it
    into another RAH tool event.
12. Codex continues reasoning and produces the final response.
```

### Prototype acceptance tests

Before any live smoke test, deterministic fake-transport tests should prove:

- initialization opts into the experimental API only for the bridge-enabled
  adapter;
- `thread/start` advertises only `echo` with the exact RAH schema;
- the existing Codex-owned capability-disabling configuration is unchanged;
- `item/tool/call` preserves JSON-RPC request ID type and responds exactly once;
- the request routes only when thread and turn match the active RAH session;
- echo arguments become `ToolInput` unchanged;
- only the real registry execution emits RAH tool lifecycle events;
- text output becomes `inputText` and `success: true`;
- unknown tool, malformed params, invalid echo input, duplicate call, permission
  denial, cancellation, and disconnect all fail closed without double execution;
- command, file-change, MCP, approval, and other experimental requests remain
  denied; and
- Codex `dynamicToolCall` item notifications do not create duplicate/fake RAH
  tool events.

An optional live test may then request a harmless echo and verify model
continuation, but it must not become part of the default workspace suite or
require credentials.

## Required adapter work before the prototype

This is a design inventory, not authorization to implement:

1. Extend private protocol parsing so server requests retain `id`, `method`, and
   `params`; support string and integer server request IDs.
2. Add a private response command/path to the connection task instead of
   immediately returning method-not-found for every request.
3. Enable `experimentalApi` only in an explicitly bridge-enabled adapter mode.
4. Give that mode an adapter-owned `Arc<ToolRegistry>` and explicit allowed
   permissions, defaulting to `PermissionLevel::None` only.
5. Snapshot and translate definitions into `thread/start.dynamicTools`, with a
   private alias map.
6. Centralize request routing by RAH session, Codex thread, and active turn.
7. Add per-turn call deduplication, cancellation ownership, response correlation,
   output/error translation, and bounded cleanup.
8. Continue rejecting every other Codex server request and prohibited tool item.
9. Extend the pinned schema contract fixture to require the experimental
   `dynamicTools`, `DynamicToolCallParams`, and `DynamicToolCallResponse` shapes.
10. Add deterministic fake-transport translation and security tests before any
    live experiment.

No production Tool Bridge or prototype is implemented by this research task.

## Contract conclusion

No architecture-defining RAH contract must change:

- `AgentRuntime` already permits an implementation to own registry and policy
  configuration outside `start`.
- `ToolDefinition` carries every model-facing function field plus RAH-only
  permission metadata.
- `ToolCall` and `ToolInput` represent the untrusted request exactly after private
  ID/name translation.
- `ToolOutput` represents the echo result and error status; JSON can be safely
  serialized into Codex text output.
- `ToolRegistry` already performs authoritative name lookup and dispatch.
- permission remains a runtime/host decision before registry execution.
- `Sandbox` remains inside applicable tool implementations.
- `AgentEvent` can describe the real RAH-owned successful echo lifecycle without
  translating Codex post-execution events.

Therefore the answer to the primary question is **yes**, subject to the
adapter-local safeguards and 0.148.0 restrictions documented above. Proceed with
an echo-only private prototype as a separate task; do not begin a general
production Tool Bridge until those tests pass and the experimental-version risks
are explicitly accepted.
