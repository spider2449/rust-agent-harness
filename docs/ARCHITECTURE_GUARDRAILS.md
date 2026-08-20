# RAH v0.1 — Architecture Guardrails

Status: Required companion specification

## 1. Crate dependency matrix

The intended dependency direction is:

```text
rah-protocol
    ^
    |
rah-core

rah-model --------> rah-protocol
rah-tools --------> rah-protocol
rah-session ------> rah-protocol
rah-sandbox ------> rah-protocol

rah-runtime ------> rah-core
             \----> rah-model
             \----> rah-tools
             \----> rah-session
             \----> rah-sandbox
             \----> rah-protocol

rah-runtime-codex -> rah-runtime
                  -> rah-protocol
                  -> Codex crates

rah-cli ----------> RAH public API/runtime abstractions
```

Rules:

- `rah-protocol` has no dependency on another RAH crate.
- Codex dependencies exist only in `rah-runtime-codex`.
- provider SDK dependencies must not enter core crates.
- dependency cycles are forbidden.
- new dependency edges require an explicit reason in the task completion report.
- convenience is not sufficient justification for reversing a dependency.

## 2. Public API stability classes

RAH APIs are classified as:

### Stable boundary candidates

These are architecture-defining extension points:

```text
AgentRuntime
ModelBackend
Tool
AgentEvent
ToolDefinition
ToolCall
ToolInput
ToolOutput
SessionStore
Sandbox
```

During v0.1 they are still pre-1.0 APIs, but Codex must not casually change their semantics or signatures.

If a numbered task appears to require a material change to one of these interfaces, stop and report:

1. why the current API is insufficient;
2. whether the issue is RAH-generic or upstream-specific;
3. the smallest compatible change;
4. alternatives that preserve the existing boundary.

### Crate-internal APIs

Private and `pub(crate)` APIs may evolve freely when the change stays inside the current task.

### Experimental APIs

Experimental public APIs must be explicitly documented as experimental and must not become dependencies of stable boundaries without review.

## 3. Protocol versioning

RAH protocol-bearing components must have an explicit protocol version before external process/plugin/server interoperability is implemented.

Reserve:

```rust
pub const RAH_PROTOCOL_VERSION: u32 = 1;
```

The exact location is chosen during the appropriate protocol task.

Future external handshakes should be able to negotiate or reject incompatible versions.

Do not create complex semantic-version negotiation in v0.1.

## 4. Conformance testing

RAH should test contracts, not only implementations.

Reserved conformance suites:

```text
RuntimeConformance
ModelBackendConformance
ToolConformance
SessionStoreConformance
```

Examples:

```text
MinimalTestRuntime --\
CodexRuntime --------+--> RuntimeConformance
NativeRuntime -------/    (future)
```

A conformance suite should verify behavior observable through the RAH-owned interface.

It must not rely on implementation-private state.

### Runtime conformance should eventually cover

- start emits a valid session;
- events are ordered coherently;
- tool calls pass through the tool boundary;
- completion emits one terminal outcome;
- cancellation does not later emit completion;
- runtime errors become RAH-owned errors.

### Model backend conformance should eventually cover

- request acceptance;
- deterministic mock streaming behavior;
- tool-call representation;
- completion termination;
- defined error behavior;
- cancellation where supported.

### Tool conformance should eventually cover

- definition is valid;
- input validation;
- deterministic error shape;
- permission metadata;
- execution produces a valid `ToolOutput`.

### Session store conformance should eventually cover

- save/load round trip;
- missing session behavior;
- update behavior;
- independent session IDs.

The default conformance suite must not require paid APIs, network access, or a real LLM.

## 5. Architecture Decision Records

Architecture-changing decisions must be recorded under:

```text
docs/adr/
```

Use an ADR when a decision changes:

- a core dependency direction;
- a stable boundary candidate;
- the runtime model;
- plugin execution model;
- security model;
- protocol compatibility strategy;
- Codex integration strategy.

Do not create ADRs for routine implementation details.

## 6. Change control

The implementation plan is authoritative for scope.

The architecture guardrails and accepted ADRs are authoritative for boundaries.

If implementation pressure conflicts with an accepted ADR, Codex must report the conflict rather than silently overwrite the decision.
