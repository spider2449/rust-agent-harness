# RAH v0.2 MCP Tool Bridge Research

Date: 2026-08-21

Status: Architecture spike only. No MCP client, server, transport, adapter crate,
configuration, or prototype is implemented by this document.

## Recommendation

**B. PROCEED WITH NEW MCP ADAPTER CRATE.**

The current public RAH contracts are sufficient for an MCP server tool to become
an ordinary `Tool` and enter the already implemented generic Codex Tool Bridge:

```text
Codex
 -> dynamic tool request
 -> rah-runtime-codex
 -> RAH ToolCall
 -> ToolRegistry
 -> MCP-backed Tool
 -> RAH-owned MCP client
 -> MCP server
 -> MCP result
 -> ToolOutput or ToolError
 -> ToolRegistry
 -> rah-runtime-codex
 -> Codex continuation
```

The MCP protocol, transports, server supervision, discovery state, and wire
errors should be isolated in a new crate, provisionally `rah-tools-mcp`. That
crate should depend upward on `rah-tools` and `rah-protocol`; neither core crate
should depend on MCP. It should expose RAH `Tool` implementations, not MCP wire
types.

The answer to question 26 is **No**: the current generic Codex Tool Bridge requires
no change to consume an MCP-backed tool. It already snapshots generic registered
definitions, privately aliases names that Codex cannot accept, enforces
host-configured RAH permission, and dispatches only through
`ToolRegistry::execute`.

This recommendation covers ordinary, completed MCP `tools/call` operations with
JSON input and text or structured-JSON output. It does not claim lossless support
for every optional MCP feature. In particular, media/resource result blocks,
interactive input-required results, tasks, sampling, elicitation, and roots are
out of scope for the first bridge and must fail closed when encountered.

## Evidence baseline

Repository evidence is authoritative:

- `AGENTS.md`, `RAH_IMPLEMENTATION_V0.1.md`, and
  `docs/ARCHITECTURE_GUARDRAILS.md`;
- accepted ADRs 0001 through 0006;
- `rah-protocol` tool types;
- `rah-tools::Tool` and `ToolRegistry`;
- the current `rah-runtime-codex` generic dynamic-tool bridge and its tests;
- `RAH_ARCHITECTURE_PLUGIN_EXTENSION.md`, `docs/ARCHITECTURE.md`, and
  `docs/SECURITY.md`; and
- all manifests and the lockfile.

Protocol context was checked against the current MCP specification, revision
`2026-07-28`. This revision is materially different from legacy initialization-
based MCP revisions: modern requests carry version and capabilities per request,
modern stdio is stateless, and modern Streamable HTTP has no protocol-level
session or standalone GET stream. An implementation should use a version-pinned
MCP SDK/protocol contract and explicitly decide whether it supports modern only
or dual-era compatibility. Relevant primary references are the MCP
[versioning rules](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning),
[tools specification](https://modelcontextprotocol.io/specification/2026-07-28/server/tools),
[stdio transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio),
[Streamable HTTP transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http),
and the [official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk/tree/main/crates/rmcp).

## 1. Existing MCP-related repository state

There is no MCP implementation in the current workspace:

- no MCP crate or module;
- no MCP client/server dependency in any manifest or `Cargo.lock`;
- no MCP configuration type or user-facing configuration file;
- no MCP transport or process supervisor;
- no MCP-specific `Tool` implementation; and
- no MCP integration or MCP server fixture in the test suite.

There are, however, intentional architecture placeholders and security gates:

- ADR 0003 says built-in, MCP, and future process-plugin tools converge into the
  same RAH-owned `Tool` and `ToolRegistry` boundary.
- `RAH_ARCHITECTURE_PLUGIN_EXTENSION.md` positions an MCP adapter between an
  external provider and `Tool`.
- `AGENTS.md`, the v0.1 plan, architecture, and security documentation reserve
  MCP as a future external-tool mechanism and forbid bypassing registry, policy,
  or sandbox authority.
- ADR 0005 keeps Codex-owned MCP disabled.
- ADR 0006 keeps Codex-owned MCP disabled even when the generic dynamic-tool
  bridge is enabled.
- `restricted_thread_params` sends an empty `mcp_servers` object to Codex.
- `rah-runtime-codex` rejects MCP elicitation requests and treats Codex
  `mcpToolCall` items as prohibited Codex-owned activity.
- Bridge tests assert that the empty Codex MCP configuration and these denial
  paths remain intact.

The generic RAH Tool Bridge is already implemented. `CodexRuntime` exposes
`connect_tool_bridge(executable, Arc<ToolRegistry>, allowed_permissions)`. At
thread start it snapshots every registered `ToolDefinition`; invalid Codex names
receive private aliases. On a dynamic request it verifies thread, turn, call,
alias, current definition, and permission, then executes the RAH `ToolCall`
through `ToolRegistry`. It also owns replay suppression, cancellation, event
publication, and Codex response translation.

Some older prose in `README.md`, `docs/ARCHITECTURE.md`, and `docs/SECURITY.md`
still describes dynamic tools as unimplemented. Current code, ADR 0006, bridge
tests, live examples, and commits `46e133a`, `bd9e81c`, and `07c58e6` are the
authoritative current state for this spike.

## 2. Sufficiency of current RAH tool contracts

The contracts are sufficient for the initial MCP-backed tool bridge:

| RAH contract | MCP need | Assessment |
| --- | --- | --- |
| `Tool` | Provider-neutral async execution | Sufficient. An `McpTool` can hold an MCP client handle plus an immutable discovered definition and implement `execute`. |
| `ToolDefinition` | Name, description, input JSON Schema, RAH permission | Sufficient for discovery, registry dispatch, permission, and Codex exposure. |
| `ToolCall` | RAH call ID, registered name, untrusted arguments | Sufficient. MCP request IDs and remote names remain adapter-private. |
| `ToolInput` | Any JSON arguments | Sufficient. Pass its `serde_json::Value` as MCP `tools/call.arguments`. |
| `ToolOutput` | Ordered text/JSON content and execution-error status | Sufficient for text and structured JSON completion results. |
| `ToolError` | Invalid input, execution/transport failure, registry failure | Sufficient for the prototype and fail-closed unsupported results. |
| `ToolRegistry` | Trait-object registration and name dispatch | Sufficient without a public change. |

The mapping is intentionally not lossless for all current MCP metadata:
`ToolDefinition` has no title, icons, annotations, or output schema, and
`ToolContent` has no image, audio, resource-link, or embedded-resource variants.
Those omissions do not prevent ordinary invocation. The first implementation
must reject unsupported result forms instead of silently discarding them.

MCP input-required results are multi-round-trip interactions rather than completed
tool results. They do not fit one `Tool::execute -> ToolOutput` call and are out of
scope. Receiving one should return a sanitized `ToolError::Execution` and never
auto-accept elicitation. Adding interactive MCP input later would require a
separate architecture review; it is not needed for the echo prototype.

## 3. MCP client ownership boundary

RAH should own the MCP client through the MCP adapter crate, not through Codex,
`rah-runtime-codex`, `rah-runtime`, or `rah-protocol`.

Conceptually the crate owns:

```text
McpServerConfig
 -> McpClientSupervisor
 -> version/capability negotiation
 -> transport and connection/process lifecycle
 -> tools/list discovery snapshot
 -> one McpTool per accepted remote definition
 -> tools/call correlation, timeout, cancellation, and result translation
```

An official SDK may implement protocol mechanics, but the RAH adapter still owns
configuration, lifecycle policy, security decisions, permission assignment,
accepted protocol versions, resource limits, and translation into RAH types.
SDK types must remain private to the crate.

## 4. New crate assessment

Create `rah-tools-mcp` in a later implementation task. Do not place MCP support in
`rah-tools`: MCP adds substantial protocol, transport, HTTP, and process-lifecycle
dependencies that built-in tools do not need. Do not place it in
`rah-runtime-codex`: that violates the critical invariant. Do not place it in
`rah-runtime`: runtimes should consume registered tools without knowing their
origin.

Proposed dependency direction:

```text
rah-protocol <--- rah-tools <--- rah-tools-mcp
                                  |
                                  +--- MCP SDK / transport dependencies

rah-runtime-codex ---> rah-tools::ToolRegistry
```

The MCP crate should provide an adapter/supervisor and `Arc<dyn Tool>` values for
host registration. A new edge `rah-tools-mcp -> rah-tools` is justified because
the crate implements the RAH-owned extension boundary. No existing crate gains a
provider-specific or MCP dependency.

## 5. MCP tool definition to `ToolDefinition`

For each accepted `tools/list` entry:

| MCP field | RAH field | Rule |
| --- | --- | --- |
| configured server ID + MCP `name` | `name` | Construct a RAH-owned namespaced identity; retain the exact remote name privately. |
| `description` | `description` | Copy when present; otherwise use a neutral generated description identifying the configured server and remote tool. |
| `inputSchema` | `input_schema` | Copy the JSON object unchanged after size/type validation. |
| host configuration | `permission` | Required RAH-owned mapping. Ignore MCP annotations for authorization. |
| `title`, `icons`, `annotations`, `outputSchema`, `_meta` | none | Do not expose through the current contract; retain only if needed privately for validation/diagnostics. |

Discovery must paginate until completion, enforce bounded tool count and schema
size, reject invalid/duplicate remote names, and produce definitions in a
deterministic order before registry construction.

## 6. `ToolCall` to MCP `tools/call`

`ToolRegistry` resolves the namespaced RAH name to one `McpTool`. Its `execute`
method performs:

```text
ToolInput(value)
 -> verify client generation is current and connected
 -> allocate private MCP JSON-RPC request ID
 -> tools/call {
      name: exact remote MCP tool name,
      arguments: value
    }
 -> await bounded result
```

Do not send the namespaced RAH name to the server. Do not send RAH permission or
trust data. MCP request correlation remains private and must not replace
`ToolCallId`.

The current MCP call schema describes arguments as an object. If `ToolInput` is
not an object, return `ToolError::InvalidInput` before sending. The remote server
must still validate the arguments against its own schema.

## 7. MCP result to `ToolOutput` or `ToolError`

Keep protocol failures distinct from completed tool execution errors:

| MCP outcome | RAH result |
| --- | --- |
| Complete result, text blocks only | `Ok(ToolOutput { Text blocks, is_error: MCP isError })` |
| Complete result with `structuredContent` | Add one `ToolContent::Json` preserving the exact JSON value. Preserve text blocks too, in their original order before the structured value. |
| Complete result with `isError: true` | `Ok(ToolOutput { ..., is_error: true })`; this is actionable tool output, not a transport failure. |
| JSON-RPC/protocol error | `Err(ToolError::Execution { sanitized message })`, with typed MCP detail retained for diagnostics inside the adapter. |
| Timeout, cancellation, disconnect, stale generation | `Err(ToolError::Execution { sanitized message })`. |
| Input-required result | Fail closed as unsupported; no elicitation or automatic response. |
| Malformed or oversized result | Fail closed as an execution/protocol failure. |

If a result includes both the backwards-compatible serialized JSON text and
`structuredContent`, preserving both may be redundant but is faithful to the
available RAH representation. The prototype returns one text block only.

## 8. JSON Schema preservation and Codex compatibility

Preserve the MCP `inputSchema` as an unchanged `serde_json::Value` in
`ToolDefinition.input_schema`. Do not rewrite `$schema`, `$defs`, `$ref`,
composition keywords, formats, or MCP `x-mcp-header` annotations merely to fit a
provider.

The current MCP revision defaults an omitted `$schema` to JSON Schema 2020-12 and
also permits an explicit draft. Codex app-server 0.148.0 independently validates
the schema accepted by its Responses dynamic-tool conversion. Therefore there
are two compatibility gates:

1. the MCP adapter accepts only a valid, bounded MCP input-schema object; and
2. the generic Codex bridge may reject a schema Codex cannot advertise.

That second mismatch is already an adapter-local Codex limitation. It does not
justify changing MCP schema or RAH public types. Preflight the complete registry
before starting a Codex thread and report incompatible definitions
deterministically. For Streamable HTTP, the MCP client must also honor the current
`x-mcp-header` validation and header-mirroring rules; stdio may ignore that
annotation.

## 9. Tool naming and collisions

Use a configured, RAH-owned stable server ID and the exact remote tool name:

```text
mcp.<server_id>.<remote_tool_name>
```

For the prototype:

```text
server_id: test
remote tool: echo
RAH ToolName: mcp.test.echo
```

Rules:

- `server_id` is configuration identity, not untrusted MCP `serverInfo.name`.
- Restrict `server_id` to lowercase ASCII letters, digits, `_`, and `-`, with a
  bounded length and no dots, so the boundary is unambiguous.
- Preserve the remote tool name exactly and case-sensitively in the proxy.
- Reserve the `mcp.` prefix for MCP-backed tools at host composition time.
- Reject duplicate configured server IDs before connecting.
- Let `ToolRegistry::register` deterministically reject any final RAH-name
  collision, including collision with a built-in explicitly using the prefix.
- Never resolve calls by suffix or by server-advertised display identity.

Dots make this RAH name invalid for direct Codex dynamic-tool naming, but the
current generic bridge already assigns a private `rah_tool_N` alias and maps it
back to the exact RAH name. Do not change `ToolName` or add MCP-specific behavior
to the Codex adapter.

## 10. Permission mapping

Permission is mandatory RAH-owned host configuration. Suggested configuration is
an explicit per-tool rule, optionally with a server-wide default:

```text
server test default_permission = deny/unconfigured
tool echo permission = PermissionLevel::None
```

Discovery metadata, descriptions, annotations, names, and schemas never grant or
lower permission. A tool without a configured permission mapping is not
registered. The prototype registers only `mcp.test.echo` with
`PermissionLevel::None`.

`CodexRuntime::connect_tool_bridge` must still receive an allowed-permissions
list containing `None`. Registration and runtime authorization are separate:
neither one implies the other. No approval request is generated or accepted.

## 11. Sandbox ownership and truthful guarantees

RAH can enforce locally:

- which MCP server configuration is enabled;
- whether a local stdio child is spawned and with which direct executable,
  arguments, environment, and working directory;
- which discovered tools are registered;
- RAH permission before `ToolRegistry` dispatch;
- call concurrency, input/output limits, timeout, cancellation attempts, and
  transport shutdown; and
- network destination and authentication configuration for HTTP.

RAH cannot generally enforce what an external MCP server does after receiving a
request. A remote server may access files, spawn processes, call networks, or
retain data according to its own privileges. A local stdio child normally
inherits OS authority unless RAH deliberately launches it through a real OS
sandbox. Process ownership, a restricted working directory, or path validation
alone is not strong isolation.

Therefore describe MCP tools as permission-gated and transport-supervised, not
"sandboxed by RAH", unless a specific deployment actually routes the server
through an enforceable `Sandbox` implementation and documents its guarantees.

## 12. Transport ownership

`rah-tools-mcp` owns transport selection and all transport-specific state.

### stdio

Support stdio first. Launch a configured program with an argument vector without
a shell; use newline-delimited JSON-RPC; keep stdout protocol-only; retain bounded
stderr for diagnostics; and own the child lifecycle. This is deterministic and
suitable for the local echo prototype.

### Streamable HTTP

Add only after stdio behavior is stable. Current Streamable HTTP uses one POST
per message and returns either JSON or request-scoped SSE. Revision `2026-07-28`
removed the standalone GET stream and protocol-level sessions. The client must
enforce HTTPS by default for non-loopback endpoints, validate configured origins,
keep credentials out of logs, set required protocol/method/name headers, honor
`x-mcp-header`, bound redirects, and never forward authorization across an origin
change.

Legacy HTTP+SSE is deprecated and should not be in the first implementation.
Dual-era support, if desired, must be explicit and tested rather than guessed.

## 13. Local stdio process lifecycle

The MCP supervisor should:

1. validate a trusted host configuration;
2. spawn one child using program plus arguments, never a shell string;
3. capture stdin/stdout and bounded stderr;
4. negotiate/probe the pinned protocol behavior;
5. discover and freeze the tool generation;
6. serve calls with bounded concurrency;
7. on graceful shutdown, stop new calls, cancel or drain active calls, close
   child stdin, wait for a bounded interval, then terminate if necessary; and
8. reap the child and publish diagnostics for abnormal exit.

On Windows, process-tree ownership should be evaluated so grandchildren are not
silently orphaned. No host configuration or package installation should occur
automatically.

## 14. Timeout behavior

Use separate bounded timeouts for connect/probe, discovery, each tool call, and
shutdown. Values are host configuration with conservative defaults and hard
maximums.

On call timeout:

- mark the private request terminal;
- issue MCP cancellation appropriate to the transport;
- return `ToolError::Execution`;
- ignore any late result; and
- do not automatically replay the call.

A timeout does not prove that the server stopped or rolled back side effects.
Escalating from one timed-out call to process restart should require repeated
health failure or a poisoned transport, not happen blindly for every timeout.

## 15. Cancellation behavior

The current Codex bridge cancels an in-flight RAH tool by aborting the spawned
execution task. `McpTool::execute` must be cancellation-safe: dropping its future
must trigger an adapter-owned request guard that cancels the private MCP request
and prevents a late response from being observed as success.

For current stdio, send `notifications/cancelled` with the MCP request ID. For
current Streamable HTTP, close the request's SSE response stream. Cancellation is
cooperative and does not undo external side effects. Do not kill a shared MCP
server solely to claim one call was cancelled unless the transport is unusable
and process termination is part of an explicit recovery policy.

## 16. Disconnect behavior

On EOF, child exit, broken pipe, HTTP disconnect, or protocol actor failure:

- fail all pending calls exactly once with transport errors;
- mark that client generation unavailable;
- reject new calls until recovery completes;
- do not convert disconnect into `ToolOutput { is_error: true }` because no valid
  MCP tool result was received; and
- never automatically replay an ambiguous in-flight call.

The outer Codex bridge will translate the resulting RAH `ToolError` through its
existing generic failure path.

## 17. Server restart behavior

Automatic restart may be offered for local stdio servers, with bounded
exponential backoff and a circuit breaker. It must never replay calls that were
in flight at failure.

After restart, re-probe and rediscover. If the accepted tool names, descriptions,
schemas, or host permission mappings differ from the frozen generation, mark the
old proxies stale and require host reconstruction of a new `ToolRegistry` and new
Codex tool-bearing thread. Only an exactly compatible generation may resume using
the existing proxies. Streamable HTTP reconnect follows the same no-replay and
generation-check rules without local process ownership.

## 18. Tool discovery and refresh

At startup, call `tools/list`, follow pagination with cycle and page/count limits,
validate each definition, apply host allow/permission rules, sort by final RAH
name, and build the registry before starting Codex.

For current MCP `tools/list_changed`, subscribe only if the chosen protocol/server
supports the required notification mechanism. A notification triggers bounded
rediscovery; it does not mutate the active registry. If the effective set or any
exposed definition changes, mark the generation stale and require a fresh
registry/runtime thread. Polling may be an explicit fallback, but there should be
no background refresh loop in the prototype.

## 19. Tool-definition mutation after registration

Do not mutate an `McpTool` definition after registration. `ToolRegistry` has no
remove/replace contract, and the Codex bridge deliberately snapshots definitions
per thread and checks that current definition content still matches.

Each proxy is immutable and tied to `(configured server ID, remote tool name,
discovery generation)`. Mutation produces a new proxy/registry/thread generation.
This works with current public contracts and avoids time-of-check/time-of-use
schema drift.

## 20. Duplicate or replayed RAH `ToolCall`

The current `ToolRegistry` does not promise global exactly-once execution, and
`Tool::execute` receives input/context rather than `ToolCallId`. Therefore an
MCP-backed tool must treat each invocation of `execute` as a new MCP request. It
cannot infer that two identical JSON inputs are the same logical call.

The current Codex bridge already deduplicates its provider call key and executes
the RAH call once. Other runtimes must provide their own call-level replay policy.
The MCP adapter must never auto-retry an ambiguous `tools/call` after timeout or
disconnect. Exactly-once semantics for arbitrary runtimes would require a broader
RAH contract decision, but it is not required for this bridge or echo prototype.

## 21. MCP protocol errors versus tool errors

Preserve this distinction internally:

- JSON-RPC parse/invalid-request/invalid-params/method/server/transport failures
  become `Err(ToolError::Execution)` with typed, sanitized adapter diagnostics.
- A valid complete `CallToolResult` with `isError: true` becomes
  `Ok(ToolOutput { is_error: true, ... })` so the model can receive actionable
  tool feedback.
- Invalid local RAH input shape before transport becomes
  `ToolError::InvalidInput`.

Do not leak credentials, environment values, command lines containing secrets,
server stderr, stack traces, or arbitrary protocol error data to the model.

## 22. MCP content conversion

Initial conversion policy:

- text content -> `ToolContent::Text`;
- `structuredContent` -> `ToolContent::Json`;
- image or audio/base64 binary content -> unsupported, fail the whole call;
- resource links -> unsupported, fail the whole call;
- embedded text or binary resources -> unsupported, fail the whole call; and
- unknown future content variants -> unsupported, fail closed.

Do not silently drop unsupported blocks or replace them with invented summaries.
Do not fetch resource links automatically. This prevents the MCP adapter from
introducing undeclared filesystem/network capability. A future full-fidelity
media/resource design may require new neutral `ToolContent` variants and thus a
separate architecture decision.

## 23. Security implications

Treat MCP server configuration and all discovered metadata/results as untrusted
capability-bearing input:

- enabling a server is an explicit host action;
- local executable paths, arguments, environment, and working directory are a
  code-execution trust boundary;
- remote URLs, redirects, DNS, TLS, authentication, and proxies are an SSRF and
  credential-exposure boundary;
- descriptions and schemas can prompt-inject the model;
- names and schemas can be huge or adversarial;
- tool results can contain malicious instructions or data-exfiltration content;
- server annotations are advisory and never permission;
- output, stderr, headers, and tracing require size limits and secret redaction;
- concurrency and rate limits must prevent resource exhaustion;
- no roots, sampling, elicitation, resources, prompts, or server-to-client
  capabilities are granted by the prototype; and
- no automatic approval, shell, or filesystem capability is introduced.

For HTTP, follow the current transport security rules and prefer allowlisted HTTPS
origins. For stdio, consider an explicit environment allowlist rather than
inheriting every parent secret.

## 24. Configuration ownership

Configuration belongs at the application/host composition boundary, with typed
MCP configuration and validation implemented in `rah-tools-mcp`:

| Layer | Responsibility |
| --- | --- |
| `rah-protocol` | None; remain MCP-free and dependency-bottom. |
| `rah-core` | None. |
| `rah-runtime` | None; consume ordinary tools. |
| `rah-runtime-codex` | None; consume an ordinary `ToolRegistry`. |
| `rah-tools` | Own only generic `Tool`/registry contracts and built-ins. |
| `rah-tools-mcp` | Typed server/transport config, client, discovery, proxies, lifecycle, mappings. |
| CLI or embedding host | Parse/select trusted config, map permissions, construct adapter and registry, choose allowed runtime permissions. |

Do not let the MCP server add or change host configuration. Do not add MCP config
to Codex's `mcp_servers`; it must remain empty.

## 25. Registration into the current `ToolRegistry`

Yes. Discovery occurs before registry sharing:

```text
let mut registry = ToolRegistry::new();
for proxy in discovered_mcp_tools {
    registry.register(proxy)?;
}
let registry = Arc::new(registry);
```

`McpTool: Tool + Send + Sync` can hold a cloneable client/supervisor handle. The
registry needs no MCP knowledge and no public contract change. Its current
duplicate-name rejection and deterministic definition sorting are useful as-is.

## 26. Changes required in the generic Codex Tool Bridge

**No.**

The bridge already consumes only:

- `ToolRegistry::definitions()`;
- RAH-owned `ToolDefinition` fields;
- current registered permission metadata;
- RAH `ToolCall`/`ToolInput`;
- `ToolRegistry::execute`; and
- RAH `ToolOutput`/`ToolError`.

An MCP-backed tool satisfies the same interface. Its dotted
`mcp.test.echo` name is handled by the bridge's existing private Codex aliasing.
Codex-owned MCP configuration remains empty and `mcpToolCall` remains prohibited.
No MCP imports, branches, DTOs, execution, lifecycle, or configuration belong in
`rah-runtime-codex`.

## Minimal prototype design

### Components

- A test-only local stdio MCP server exposing exactly one deterministic `echo`
  tool. It accepts `{ "text": string }` and returns the same text.
- No filesystem, shell, network, roots, resources, prompts, sampling,
  elicitation, or approval capability.
- A future `rah-tools-mcp` client using a pinned protocol/SDK contract.
- Server configuration ID `test`.
- One discovered proxy named `mcp.test.echo` with
  `PermissionLevel::None` supplied by RAH configuration.
- A `ToolRegistry` containing only that proxy.
- The existing `CodexRuntime::connect_tool_bridge` with allowed permission
  `None`.

Prefer implementing the test server in-process as a dedicated test binary or
fixture from the same MCP SDK, launched over stdio by direct executable path. Do
not depend on `npx`, package installation, the network, a shell, or a third-party
filesystem-capable server.

### Expected path

```text
1. RAH starts the local echo MCP server and discovers remote `echo`.
2. RAH maps it to immutable `mcp.test.echo` and registers the `McpTool`.
3. The existing Codex bridge snapshots the registry and privately aliases the
   dotted name for Codex.
4. Codex requests the private dynamic-tool alias.
5. The generic bridge resolves it to a RAH ToolCall named `mcp.test.echo`.
6. Existing RAH permission policy accepts PermissionLevel::None.
7. ToolRegistry executes the MCP-backed Tool without knowing its implementation.
8. The MCP-backed Tool sends `tools/call(name = "echo")` to the local server.
9. The server returns one text result; the adapter creates RAH ToolOutput.
10. The generic bridge performs its existing ToolFinished and Codex response
    translation; Codex continues and completes.
```

At no point does Codex know the RAH tool is MCP-backed. Codex never connects to
the server, and `rah-runtime-codex` never becomes an MCP client.

### Deterministic acceptance tests

Before an optional live Codex smoke test, prove without network/model access:

- discovery maps the exact echo schema to `mcp.test.echo`;
- permission comes only from RAH configuration and is `None`;
- the proxy registers in the unchanged `ToolRegistry`;
- a RAH call becomes MCP `tools/call` with exact remote name and arguments;
- MCP text output becomes the expected `ToolOutput`;
- protocol error and `isError: true` remain distinct;
- timeout, cancellation, child exit, malformed result, oversized result, and
  unsupported content fail closed;
- duplicate server/tool/final RAH names are rejected deterministically;
- tool-list mutation makes the generation stale rather than mutating a live
  definition;
- no ambiguous call is replayed after disconnect/restart;
- the existing generic Codex fake-transport tests pass unchanged with an
  `McpTool` in the registry; and
- Codex `mcp_servers` remains empty and Codex-owned MCP requests/items remain
  denied.

An optional live smoke may then prove the complete continuation path, but it must
not enter the default suite or require credentials. No production MCP integration
should be implemented as part of this research task.

## Architecture and ADR conclusion

This design implements ADR 0003's already accepted extension direction and
preserves ADRs 0001, 0002, 0005, and 0006. It adds no provider type to a RAH
public boundary, no dependency to `rah-protocol`, no MCP behavior to the Codex
adapter, and no bypass around permission or `ToolRegistry`.

A later implementation should record the selected MCP protocol versions,
transport support, lifecycle policy, and crate boundary in a new ADR because
those become compatibility and security commitments. No accepted ADR needs to be
changed for the prototype.

Final recommendation: **B. PROCEED WITH NEW MCP ADAPTER CRATE.**
