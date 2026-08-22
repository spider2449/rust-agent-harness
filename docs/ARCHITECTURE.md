# RAH v0.3 Architecture

## Ownership boundaries

RAH public boundaries use only RAH-owned neutral types. `rah-protocol` is the
dependency-bottom crate and contains serializable identifiers, messages, events,
tool descriptions, calls, and outputs. Provider, runtime, MCP, and process-plugin
adapters translate only at their private edges.

`AgentRuntime`, `ModelBackend`, `Tool`, `ToolRegistry`, `SessionStore`, and
`Sandbox` remain independent extension points. No v0.3 work changes their
architecture-defining public contracts.

ADR 0011 establishes the trusted host capability profile as the authority-
composition boundary for existing built-in capabilities and admitted external
providers. It does not change runtime, `Tool`, `ToolRegistry`, or
capability-specific policy contracts.

## v0.3 capability classification

### Public / host capabilities

The host-owned Execute surface is limited to `host.cargo.version`,
`host.git.status`, `host.git.stage`, and `host.git.unstage`. The first two are
fixed, host-constructed inspection capabilities. Stage and unstage use the
private `RepositoryMutationPolicy` to prove one authorized index-only effect
for one host-selected target. They never grant generic process, worktree-byte,
history/ref, network, or credential authority.

### Validation fixtures

The hardened Execute deterministic/live fixture (`process.test.echo`) and the
repository-mutation deterministic/live fixture are validation infrastructure.
They establish policy behavior before the public host capabilities are exposed;
they are not production/public capabilities. In particular,
`host.fixture.echo` does not exist.

The Generic Tool Bridge, `fs.read`, MCP adapter, and process-plugin adapter are
also verified v0.3 components. All converge through RAH-owned `Tool`,
`ToolRegistry`, and permission interfaces.

## Current crate topology

Production RAH dependency edges are:

```text
rah-core                                  (no RAH dependencies)
rah-sandbox                               (no RAH dependencies)

rah-protocol                              (dependency bottom)
  ^        ^          ^          ^
  |        |          |          |
model   session      tools     runtime
                     ^  ^         ^
                     |  |         |
             tools-mcp  tools-plugin

rah-tools   -> rah-protocol, rah-sandbox
rah-runtime -> rah-model, rah-protocol, rah-tools
rah-runtime-codex -> rah-protocol, rah-runtime, rah-tools
rah-cli     -> rah-model, rah-protocol, rah-runtime, rah-tools
```

`rah-tools-mcp` and `rah-tools-plugin` each depend on `rah-protocol` and
`rah-tools`. They do not depend on a runtime. `rah-runtime-codex` has no
production dependency on either adapter crate and contains no MCP- or
plugin-specific dispatch. Its manifest uses them only as dev dependencies for
the opt-in examples and cross-boundary tests.

## Tool convergence

Every tool source converges before runtime dispatch:

```text
Built-in Tool -----------\
MCP Tool -----------------+-> Tool -> ToolRegistry
Process Plugin Tool ------/
```

The registry is unaware of transport or provider. It stores `Arc<dyn Tool>`,
returns deterministic definition snapshots, rejects duplicate names, and
dispatches parsed `ToolCall` values. Host composition selects adapters, assigns
permissions, and registers their proxies.

`ExternalToolIdentity` is opaque and provider-neutral. The host uses
`ExternalToolPermissionPolicy` to assign a `PermissionLevel` to each discovered
identity. Missing assignments fail closed before registration; server/plugin
metadata cannot grant authority.

## Deterministic runtime

`MinimalTestRuntime` proves the provider-neutral loop using `MockBackend`. Its
default host policy allows only `PermissionLevel::None`; the manifest demo
explicitly adds `Read`, while `FsReadTool` independently enforces its configured
workspace boundary.

## Generic Codex Tool Bridge

`rah-runtime-codex` owns these private layers:

```text
CodexRuntime
 -> optional generic RAH Tool Bridge
 -> session/thread/turn translation
 -> correlated connection actor
 -> private JSON-RPC parsing
 -> stdio transport
 -> owned codex app-server child
```

The executable must report `codex-cli 0.148.0`. The adapter generates the
installed app-server schema locally and verifies the required lifecycle fields;
bridge mode additionally verifies the version-pinned experimental dynamic-tool
contract.

Bridge mode snapshots any host-supplied `ToolRegistry` for a new Codex thread.
It advertises provider-private aliases where RAH tool names are not accepted by
Codex, translates a valid request into the original RAH `ToolCall`, checks the
host's allowed permission levels, dispatches through the registry, emits RAH
tool lifecycle events, and returns the translated result. Dedupe, replay,
cancellation, correlation, and call bounds remain adapter-private.

The bridge does not know whether a registered tool is built-in, MCP-backed, or
process-plugin-backed. Codex-owned shell, file, MCP, web, image, app, and approval
capabilities remain disabled even in bridge mode.

## External process adapters

`rah-tools-mcp` owns the pinned MCP `2025-06-18` stdio handshake, discovery,
request correlation, timeout, cancellation, result conversion, child ownership,
and immutable `mcp.<server>.<tool>` proxies.

Trusted static profile composition is host-only. Its static pass parses closed
symbolic MCP declarations without launching a provider; explicit effective
composition delegates construction and exact admission to `rah-tools-mcp`, then
publishes a fresh `ToolRegistry` only when every provider has validated. The
effective profile retains adapter ownership for as long as its tools are usable.

`rah-tools-plugin` owns RAH process-plugin protocol version `1`, identity and
version validation, bounded NDJSON stdio, resource limits, process lifecycle,
and immutable `plugin.<plugin>.<tool>` proxies. It is a focused adapter, not a
general plugin manager, installer, marketplace, SDK, or dynamic-library ABI.

## Conformance and architecture gates

Generic deterministic conformance helpers cover observable `ModelBackend`,
`Tool`, `SessionStore`, and `AgentRuntime` contracts. Adapter tests use local
fixtures and fake Codex transport. Architecture gates prevent Codex Rust
dependencies, provider dependencies in core crates, upward dependencies from
`rah-protocol`, and escaped Codex implementation details. The production
manifest keeps MCP and process-plugin adapters out of `rah-runtime-codex`.
