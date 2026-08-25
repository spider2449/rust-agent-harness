# RAH v0.8 Milestone Architecture

## Ownership boundaries

RAH public boundaries use only RAH-owned neutral types. `rah-protocol` is the
dependency-bottom crate and contains serializable identifiers, messages, events,
tool descriptions, calls, and outputs. Provider, runtime, MCP, and process-plugin
adapters translate only at their private edges.

`AgentRuntime`, `ModelBackend`, `Tool`, `ToolRegistry`, `SessionStore`, and
`Sandbox` remain independent extension points. No v0.4 work changes their
architecture-defining public contracts.

ADR 0011 establishes the trusted host capability profile as the authority-
composition boundary for existing built-in capabilities and admitted external
providers. It does not change runtime, `Tool`, `ToolRegistry`, or
capability-specific policy contracts.

ADR 0012 establishes a distinct private, host-owned
`RepositoryWorktreeMutationPolicy` for the bounded `repo.patch` capability.
It is deliberately separate from ADR 0010's index-only
`RepositoryMutationPolicy` and ADR 0011's composition-only profile boundary:

```text
worktree content mutation != index mutation != history/ref mutation
```

ADR 0013 establishes a separate private, host-owned
`RepositoryFileCreationPolicy` for `repo.create-file`. It authorizes only one
exclusive creation of one absent UTF-8 file under a host-bound repository; it
does not grant generic `fs.write`, overwrite, directory creation, staging, or
index/history/ref authority. `repo.patch` and `repo.create-file` share the
same per-repository mutation lease, so they cannot independently widen mutation
concurrency or authority.

The public `RepositoryWorktreePatchTool` constructor fixes a host-selected Git
executable and repository root. Its closed request schema permits only a
logical relative path, complete-file SHA-256 and byte-length preconditions, one
nonempty literal expected text, and replacement text. The private policy
performs all repository eligibility, identity, replacement, and uncertainty
handling; model requests and `PermissionLevel::Execute` remain insufficient
authority by themselves.

The repository-observation tools are four separate, host-constructed tools:
`repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`. They use
a crate-private fixed-command observer envelope with host-selected executable
and repository identities, fixed cwd/environment, bounds, and the existing RAH
repository lease. `Execute` remains an outer process gate, not generic Git,
filesystem, or mutation authority. This bounded observation work neither
extends ADR 0010 index mutation nor ADR 0012 worktree mutation; ADR 0011 alone
governs trusted-profile composition.

## Trusted capability profile

ADR 0011 defines trusted profile source validation and the explicitly selected
trusted static profile as a host-only authority-composition boundary. The profile
selects existing constructors, symbolic resources, exact external-provider
admission, and explicit permission mappings; it is neither an `AgentRuntime`
nor a model-facing API.

```text
Trusted Host Capability Profile
             |
             v
      effective composition
             |
             v
         ToolRegistry
       /      |       \
 built-in    MCP     Plugin
```

Static validation parses and validates the source/profile/resources without
launching a provider. Explicit effective composition resolves host-owned
resources, launches configured providers where required, admits their exact
declared tools/schemas, and returns a fresh registry only after complete
success. The effective profile owns its provider adapters for the registry's
usable lifetime.

The runtime remains downstream of this boundary. `CodexRuntime` receives a
host-supplied registry through its optional Generic Tool Bridge and cannot
select providers or profile authority.

## Preserved v0.3 capability classification

### Public / host capabilities

The host-owned Execute surface includes `host.cargo.version`, `host.git.status`,
`host.git.stage`, `host.git.unstage`, `repo.create-file`, and the fixed observers
`repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`. The first
two and the observers are fixed, host-constructed inspection capabilities.
Stage and unstage use the private `RepositoryMutationPolicy` to prove one
authorized index-only effect for one host-selected target. They never grant
generic process, worktree-byte,
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

Every tool source converges through the profile/composition boundary before
runtime dispatch:

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

The executable must report `codex-cli 0.149.0`. The adapter generates the
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
symbolic MCP and Process Plugin declarations without launching a provider;
explicit effective composition delegates construction and exact admission to
their hardened adapters, then publishes a fresh `ToolRegistry` only when every
provider has validated. Provider-qualified names and registry duplicate checks
fail closed where names could otherwise collide. The effective profile retains
adapter ownership for as long as its tools are usable.

`rah-tools-plugin` owns RAH process-plugin protocol version `1`, identity and
version validation, host-selected executable identity checks, exact expected
tool/schema admission, bounded NDJSON stdio, resource limits, process
lifecycle, and immutable `plugin.<plugin>.<tool>` proxies. Admission builds
privately and publishes only after the complete provider validates. It is a
focused adapter, not a general plugin manager, installer, marketplace, SDK, or
dynamic-library ABI. It is a trusted-profile provider only through the closed
ADR 0011 `process_plugins` declaration and the host-owned effective composer.

## Conformance and architecture gates

Generic deterministic conformance helpers cover observable `ModelBackend`,
`Tool`, `SessionStore`, and `AgentRuntime` contracts. Adapter tests use local
fixtures and fake Codex transport. Architecture gates prevent Codex Rust
dependencies, provider dependencies in core crates, upward dependencies from
`rah-protocol`, and escaped Codex implementation details. The production
manifest keeps MCP and process-plugin adapters out of `rah-runtime-codex`.
