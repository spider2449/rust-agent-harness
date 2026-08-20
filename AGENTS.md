# AGENTS.md — RAH Repository Instructions

## Project identity

This repository contains **RAH — Rust Agent Harness**.

RAH is a model-agnostic, runtime-pluggable agent harness written in Rust.

The authoritative implementation plan for v0.1 is:

```text
RAH_IMPLEMENTATION_V0.1.md
```

Read it before implementing any numbered task.

---

## Primary development environment

RAH v0.1 is primarily developed through **Codex Desktop operating on a local Git repository**.

Treat the local repository as the source of truth.

Codex Desktop is development tooling; it is not part of the RAH runtime architecture. Do not add dependencies on Codex Desktop APIs, UI state, installation layout, or application-specific behavior.

All implementation must remain compatible with normal local `cargo`, `rustc`, and `git` workflows.

Before editing, inspect the working tree and preserve existing user changes.

Unless explicitly instructed:

- edit only inside the current repository;
- do not modify host/system configuration;
- do not install system-wide packages;
- do not modify unrelated repositories;
- do not push, merge, rebase, or reset;
- do not discard user changes;
- do not change Git remotes;
- do not force-clean the working tree.

If a required tool or system dependency is missing, report it rather than silently changing the machine.

After each task inspect:

```bash
git status --short
git diff
```

and summarize both in the completion report.

---

## Instruction precedence inside this repository

This root `AGENTS.md` defines the baseline rules for the entire repository.

If a deeper directory contains another `AGENTS.md`, it may add local guidance for that subtree, but it must not weaken the architectural invariants defined here unless an explicit human instruction says otherwise.

Do not create nested `AGENTS.md` files unless a task explicitly calls for them or the subtree truly requires distinct local rules.

---

## Core architectural invariants

### 1. RAH is not Codex

Codex may be used as an optional runtime implementation.

Only this crate may directly depend on Codex crates:

```text
crates/rah-runtime-codex
```

No other crate may import `codex_*` crates or expose Codex types.

If integrating Codex appears to require changing RAH public abstractions, report the mismatch first.

Do not redesign RAH merely to mirror Codex internals.

### 2. RAH is model-provider agnostic

Core crates must not contain provider-specific control flow.

Forbidden examples in core code:

```rust
if provider == "openai" { ... }
if provider == "deepseek" { ... }
```

Provider-specific code belongs behind `ModelBackend` implementations/adapters.

### 3. No inference engine

Do not implement:

- transformer inference;
- model weight loading;
- CUDA kernels;
- ROCm kernels;
- Metal kernels;
- KV cache;
- tokenizer internals.

RAH orchestrates inference providers; it does not become one.

### 4. Model output is untrusted

Never execute model-generated actions directly.

Required conceptual path:

```text
Model output
 -> parsed ToolCall
 -> ToolRegistry
 -> permission/policy
 -> sandbox/executor
 -> Tool
 -> ToolOutput
```

Do not bypass permission or sandbox boundaries for convenience.

### 5. Stable RAH types own public boundaries

Public RAH APIs must use RAH-owned neutral types.

Do not expose:

- Codex SDK types;
- OpenAI SDK types;
- Anthropic SDK types;
- DeepSeek-specific types;
- provider-specific wire structs.

Translation belongs in adapters.

---

## Architecture guardrails and ADRs

Before changing architecture-defining code, read:

```text
docs/ARCHITECTURE_GUARDRAILS.md
docs/adr/
```

Accepted ADRs are repository decisions, not suggestions.

Architecture-defining extension points include:

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

These are pre-1.0 APIs, but do not materially change their signatures or semantics merely to simplify one implementation.

If a task appears to require such a change and the plan does not already authorize it, stop and report the conflict.

New crate dependency edges require an explicit reason in the task completion report.

`rah-protocol` must remain dependency-bottom among RAH crates.

Conformance tests should target observable behavior through RAH-owned interfaces and should be reusable across implementations.

---

## Plugin extension point

RAH must preserve a future plugin extension point, but v0.1 does not implement a general plugin platform.

The architectural rule is:

```text
Built-in Tool
MCP Tool
Future Process Plugin Tool
        |
        v
     Tool trait
        |
        v
   ToolRegistry
```

All external tool providers must adapt into the same RAH-owned `Tool` abstraction.

While implementing v0.1:

- do not assume all tools are compiled into the binary;
- do not make AgentRuntime depend on concrete built-in tool types;
- keep `ToolDefinition`, `ToolCall`, `ToolInput`, and `ToolOutput` provider-neutral and transport-neutral;
- keep permission/policy enforcement outside plugin authority;
- do not allow plugins to bypass `ToolRegistry`, policy, or sandbox boundaries;
- do not expose `AgentContext`, `Session`, `AgentRuntime`, or `ModelBackend` internals as plugin APIs;
- do not implement Rust dynamic library loading unless explicitly required in a future plan;
- prefer future process isolation / IPC / MCP over Rust ABI coupling.

A future plugin may expose tools. It does not own or redefine RAH core runtime behavior.

If a current implementation choice would make future MCP/process-plugin tools impossible to register through the same `ToolRegistry`, stop and report the architectural conflict.

Reserved future components such as `PluginManager`, `PluginManifest`, plugin discovery, plugin installation, and plugin SDK are out of scope for v0.1 unless explicitly requested.

---

## Scope discipline

Work on exactly the requested task.

Do not implement future tasks preemptively.

Do not add speculative abstractions unless they are required to complete the current task while preserving architecture.

Before modifying code:

1. read the current task;
2. inspect relevant files;
3. inspect applicable `AGENTS.md` files;
4. identify dependency boundaries;
5. choose the smallest coherent change.

If a task is ambiguous but does not affect architecture, choose the simplest implementation consistent with existing code and tests.

If ambiguity materially affects architecture, stop and report it.

---

## Rust conventions

Use idiomatic stable Rust.

Prefer:

- explicit domain types;
- small modules;
- narrow public APIs;
- `thiserror` for library errors;
- `anyhow` only at application boundaries where appropriate;
- `tracing` for diagnostics;
- `tokio` for async runtime;
- `serde` for protocol serialization;
- strong typed IDs over bare strings.

Avoid:

- unnecessary `unwrap()` / `expect()` in production paths;
- hidden global mutable state;
- broad wildcard imports in public-facing modules;
- provider-specific conditionals in core;
- giant modules;
- speculative generics;
- macros when normal Rust is clearer.

When `format!` can use inline captured variables, prefer:

```rust
format!("failed to load {path}")
```

over:

```rust
format!("failed to load {}", path)
```

---

## Error handling

Library crates should return typed errors.

Do not erase domain errors into `anyhow::Error` inside reusable library APIs.

Use `anyhow` primarily in:

```text
rah-cli
examples
one-shot application boundaries
```

Errors should retain enough structured information for callers to react correctly.

---

## Async rules

Long-running operations must be designed for cancellation.

Do not:

- block Tokio worker threads with long synchronous work;
- hold mutex guards across `.await` unless explicitly justified;
- spawn detached tasks without lifecycle ownership;
- silently swallow task failures.

Prefer explicit ownership and propagation.

---

## Protocol rules

`rah-protocol` must remain low-dependency and business-logic-light.

It must not depend on:

- other RAH crates;
- Codex;
- HTTP clients;
- Tokio unless a future architectural decision explicitly requires it;
- provider SDKs.

Protocol structs should be serializable where they cross process/API boundaries.

Do not add provider-specific fields to shared protocol objects merely because one provider supports them.

---

## Dependency rules

Before adding a dependency:

1. confirm standard library or an existing workspace dependency does not already solve the need;
2. confirm it is required by the current task;
3. prefer mature, focused crates;
4. avoid heavy frameworks for small functionality.

Use workspace dependencies where appropriate.

Do not add a dependency solely for a future task.

---

## Testing rules

Every behavior change must include appropriate tests.

Prefer deterministic tests.

The default test suite must not require:

- internet access;
- paid API credentials;
- a real LLM;
- GPU hardware.

Use `MockBackend` or fixtures for agent behavior.

Integration tests are preferred for cross-component agent flow.

Unit tests are appropriate for:

- parsing;
- validation;
- registry behavior;
- policy behavior;
- serialization;
- pure transformations.

Do not add test-only branches to production logic when a fixture/helper can solve the problem.

---

## Codex adapter testing

Changes under:

```text
crates/rah-runtime-codex
```

must test translation boundaries where possible:

```text
RAH request -> Codex request
Codex event -> RAH event
Codex errors -> RAH errors
Codex tool calls -> RAH tool calls
```

Do not require a live model for the normal workspace test suite.

Treat Codex upstream API changes as adapter-local breakage.

---

## Security rules

Filesystem operations must respect configured workspace boundaries.

Path traversal and symlink behavior must be tested where relevant.

For subprocess execution:

- prefer program + argument vectors;
- do not default to shell-string interpolation;
- validate working directories;
- support timeout;
- retain stdout/stderr/exit status;
- route execution through the sandbox/executor abstraction.

Never label a path-check-only mechanism as strong OS isolation.

Be precise about the security guarantees actually implemented.

---

## Change size

Keep changes reviewable.

For non-mechanical work:

- target fewer than 500 changed lines;
- avoid exceeding 800 changed lines;
- if larger, split into coherent stages.

Do not combine refactoring, new architecture, and feature implementation in the same task unless required.

---

## Formatting and validation

Before declaring a task complete, run relevant checks.

At milestone boundaries run:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

For an individual crate, targeted commands may be run first.

Do not claim success without reporting the commands actually run and their results.

---

## Commit discipline

One numbered implementation task should normally map to one coherent commit.

Suggested commit prefixes:

```text
chore:
feat:
fix:
refactor:
test:
docs:
```

Do not create a commit unless the user/task environment explicitly allows commits.

If commits are not requested, still keep the patch logically commit-sized.

---

## Documentation rules

Architecture comments should explain WHY a boundary exists, not restate obvious Rust syntax.

Public types that form RAH extension points should have concise rustdoc.

Update documentation when a public contract changes.

Do not change the architecture document to justify an implementation shortcut.

---

## Forbidden shortcuts

Do not:

- put Codex dependencies into `rah-core`;
- put provider SDK types into `rah-protocol`;
- execute model-generated shell directly;
- bypass policy checks;
- introduce an inference engine;
- add SQLite before the implementation plan reaches that phase;
- build a TUI in v0.1;
- add RAG/vector memory in v0.1;
- introduce multi-agent orchestration in v0.1;
- add DCC-specific code to core;
- silently broaden the requested task.

---

## When to stop and report instead of coding

Stop and report a conflict when completing a task would require any of the following:

- breaking a core architectural invariant;
- exposing Codex/provider-specific types through RAH public APIs;
- changing a public trait merely to mirror an upstream API;
- weakening a security boundary;
- introducing a new subsystem not present in the plan;
- making a nondeterministic external service mandatory for tests;
- adding substantial scope from a later task.

A conflict report should contain:

1. the requirement that conflicts;
2. the existing architectural rule;
3. the smallest options available;
4. the recommended option;
5. no speculative code changes unless requested.

---

## Task completion report

After each task, report:

```text
Task:
Summary:
Files changed:
Tests added/updated:
Commands run:
Results:
Git status:
Diff summary:
New dependency edges:
ADR impact:
Architecture deviations:
Remaining risks:
Suggested next task:
```

`Architecture deviations` should normally be:

```text
None.
```

AUTONOMOUS EXECUTION MODE

When instructed to execute the RAH v0.1 implementation plan,
continue through sequential tasks autonomously.

For each task:

1. inspect
2. implement
3. test
4. self-review
5. validate architecture
6. inspect git diff/status
7. commit if clean
8. continue to the next task

Do not require human approval between normal tasks.

STOP immediately if:

- an accepted ADR must change;
- a stable architecture boundary must change;
- Codex types would leak outside rah-runtime-codex;
- a provider-specific dependency would enter RAH core;
- security must be weakened;
- a task requires destructive Git operations;
- required tests cannot pass without changing architecture;
- requirements are materially contradictory;
- implementation would exceed the authorized phase.

---

## v0.1 north star

A successful RAH v0.1 must demonstrate this flow without relying on a specific model vendor:

```text
User input
 -> AgentRuntime
 -> ModelBackend
 -> ToolCall
 -> ToolRegistry
 -> Permission/Sandbox
 -> Tool
 -> ToolOutput
 -> ModelBackend
 -> AgentEvent stream
 -> Final output
```

And this dependency property must remain true:

```text
RAH public architecture
       |
       +-- CodexRuntime (optional adapter)
       +-- future NativeRuntime
       +-- future other runtimes
```

Codex accelerates RAH.

Codex does not define RAH.
