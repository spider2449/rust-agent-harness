# ADR 0011 — Trusted host capability profile authority boundary

Status: Accepted

## Context

RAH is model-provider-agnostic and runtime-pluggable. Its existing security
model treats model output as an untrusted request that must pass through the
RAH-owned `ToolRegistry`, permission decision, and applicable policy or sandbox
before a `Tool` executes.

Tasks 028 through 030 implemented an explicitly selected trusted static profile
source, source validation, bounded UTF-8 reading, strict parsing and versioning,
symbolic host-resource resolution, built-in capability construction, fresh
`ToolRegistry` construction, and a redacted effective inventory. Task 035
extended that composition to local stdio MCP providers through hardened adapter
construction, exact expected discovery/schema admission, explicit external-tool
permission mapping, immutable RAH tool proxies, and atomic registration.

The profile now composes both built-in RAH capabilities and external MCP tool
providers from one host-controlled input. That is a durable authority boundary,
not merely a configuration convenience. Earlier v0.3 research noted that an
ADR 0011 would be needed before destructive worktree mutation. That ADR was
never drafted; this different architecture boundary is implemented first, so
the available number is assigned here.

## Decision

RAH uses an explicitly selected, trusted-host capability profile as the
composition boundary for determining which existing capabilities and external
tool providers a host instance may expose. The profile may select and bind
host-owned resources and permissions to already approved constructors. It does
not create a new class of authority, grant model authority, or bypass
capability-specific security policy.

The authority path is:

```text
trusted host
 -> capability profile
 -> validated host resources
 -> existing capability/provider constructors
 -> capability-specific policies
 -> ToolRegistry
 -> runtime/model-visible Tool definitions
```

### Host authority source and explicit selection

Profile loading is a trusted-host action. Model input, repository content, MCP
metadata, plugin metadata, tool descriptions, tool schemas, and generated
runtime or model output cannot select, grant, or modify profile authority.
Repository-local profile auto-discovery is absent unless separately authorized
by a future decision.

The current source is one explicitly selected absolute trusted static profile
path. Its loader performs hardened source validation before strict parsing:
the input is a bounded UTF-8 regular file and links/reparse points and
unsupported path forms are rejected. This is not a claim of full ACL ownership,
trusted-store integrity, or race-free filesystem identity.

### Composition is bounded by existing authority

Profile entries configure only existing, approved capability or provider
constructors. An entry does not independently authorize arbitrary process or
shell execution, filesystem mutation, Git history/ref mutation, network access,
or credentials. A schema field cannot invent an authority class; a new class
continues to require its own design and security review, and an ADR where
appropriate.

The profile does not replace `PermissionLevel`,
`ExternalToolPermissionPolicy`, `HostExecutionPolicy`,
`RepositoryMutationPolicy`, `WorkspacePolicy`, or provider-specific admission
and hardening. Profile configuration, the relevant capability-specific policy,
and `ToolRegistry` permission enforcement must all succeed.

Where practical, the profile names logical symbolic host resources rather than
exposing raw process authority. A symbolic resource identity is distinct from
the resolved host-native resource identity. Resolved paths and policy internals
are not model-visible.

### Static and effective validation

`rah profile validate` performs non-spawning static validation only. It loads
the selected profile and reports a redacted static inventory; it does not launch
an external provider.

`rah profile validate-effective` explicitly performs effective composition and
may launch the configured external providers. These commands intentionally have
different semantics and must not be collapsed into one validation operation.

### External provider admission and atomicity

For MCP, effective composition requires all of the following:

```text
trusted provider configuration
 -> native executable validation
 -> hardened process launch
 -> exact expected tool-set discovery
 -> exact normalized expected schema
 -> explicit permission mapping
 -> atomic admission
```

Discovered metadata is untrusted and cannot grant permission. Unexpected tools
are rejected rather than silently admitted.

A profile is composed into a fresh registry. If any configured capability or
provider fails validation, RAH publishes no replacement `ToolRegistry`, no
effective inventory, and no partially admitted provider set. An effective
profile retains lifecycle ownership of every provider needed by its proxy tools;
tools must not reference a destroyed provider connection.

### Inspection, immutability, and failure behavior

Trusted operators may inspect a redacted effective inventory containing symbolic
capability/provider/tool identity, permission, validation status, and registered
state. It must not expose unnecessary absolute paths, argv, cwd, environment,
stderr, credentials, secrets, temporary directories, or private policy internals.
The inventory is a host-facing diagnostic surface, not a model-visible authority
surface.

Current profile configuration is immutable for the process/effective-composition
lifetime. Automatic reload and hot reload are deferred. A future reload design
must not silently expand authority during an active session.

Unknown, ambiguous, malformed, unsupported, or unconfigured authority fails
closed. This includes unknown capabilities/providers/symbolic resources,
unsupported versions, missing external permissions, unexpected discovered tools,
schema mismatches, and provider validation failures.

The profile is not a generic subprocess-launch language. Provider-specific
schemas are preferred over a generic executable-plus-argv-plus-cwd-plus-env
definition, so capability-specific boundaries remain reviewable and enforceable.

## Implemented scope

- Built-in trusted static profile composition.
- Hardened trusted profile source validation.
- Redacted effective inventory.
- `rah profile validate`.
- `rah profile validate-effective`.
- Local stdio MCP trusted-profile composition.
- Exact MCP expected tool and schema admission.
- Explicit MCP permission mapping.

## Deferred scope

- Process Plugin profile composition.
- MCP Streamable HTTP and network MCP.
- Profile auto-discovery and hot reload.
- `PluginManager`.
- Provider installation or download.
- Generic provider and generic subprocess schemas.
- A new model-facing profile API.

## Relation to earlier ADRs

This ADR extends rather than replaces ADRs 0001 through 0010:

- ADR 0001 still makes `AgentRuntime` RAH-owned; profile composition does not
  change the runtime abstraction.
- ADR 0002 still keeps Codex optional and adapter-local; profiles expose
  RAH-owned tools, not Codex authority or types.
- ADR 0003 remains the extension boundary: built-in and external providers
  converge through `Tool` and `ToolRegistry`.
- ADR 0004 remains unchanged: profiles orchestrate capabilities and providers,
  not inference.
- ADRs 0005 and 0006 retain the restricted Codex app-server and dynamic-tool
  bridge boundaries; a profile does not enable Codex-owned capabilities.
- ADR 0007 remains the MCP adapter decision. This ADR composes its approved
  RAH tools; MCP still adapts into `Tool` and retains adapter-local hardening.
- ADR 0008 remains the process-plugin adapter decision. Process Plugin profile
  composition is explicitly not implemented here.
- ADR 0009 retains `HostExecutionPolicy` as separate, capability-specific
  Execute authority; a profile entry is not Execute authorization.
- ADR 0010 retains `RepositoryMutationPolicy` as separate, capability-specific
  mutation authority; a profile entry is not repository-mutation authorization.

## Consequences

Positive consequences:

- RAH has one host-owned authority-composition seam for built-in and external
  tool registration.
- External permissions remain explicit and host-assigned.
- Effective authority is inspectable without becoming model-visible topology.
- Hosts can configure existing capabilities without allowing model-controlled
  execution parameters.
- Activation is atomic rather than a partially active interpretation of a
  requested profile.

Costs and constraints:

- The profile schema is a durable compatibility surface.
- Every provider kind needs hardening and exact admission before it may compose.
- Effective validation may spawn trusted configured external providers.
- Source and resource identity retain platform-specific residual filesystem and
  process races.
- Adding a provider kind requires deliberate schema and security review.

## Alternatives rejected

### Model-configured capabilities

Rejected because a model request is untrusted data, not host authorization.

### Repository-local implicit configuration

Rejected for the current design because repository content must not silently
expand host authority.

### Generic subprocess profile

Rejected because it collapses capability-specific boundaries into generic
process authority.

### Partial provider activation

Rejected because it would make the effective authority set differ from the
trusted profile's requested and fully validated set.

### Provider metadata-based permissions

Rejected because discovered provider metadata is untrusted and must not grant
host authority.

### Automatic hot reload

Deferred because authority could change while active sessions retain stale
assumptions and tool registries.

## Security non-guarantees

Trusted profile composition does not imply OS sandboxing, network isolation,
perfect ACL ownership, race-free executable/source identity, rollback, or
protection from all external filesystem/process races. Provider process
supervision remains distinct from sandboxing.
