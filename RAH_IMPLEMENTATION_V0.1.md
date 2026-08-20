# RAH v0.1 — Codex Implementation Plan

Status: Draft for implementation
Target: RAH v0.1
Project: RAH — Rust Agent Harness
Primary language: Rust
Execution model: Task-by-task, reviewable commits
Primary agent: OpenAI Codex

---

## 0. Purpose

This document converts the RAH architecture into an implementation sequence that Codex can execute safely.

RAH is a model-agnostic, runtime-pluggable agent harness written in Rust.

RAH is NOT:

- an inference engine;
- a DeepSeek-specific application;
- an OpenAI-specific application;
- a Codex fork by definition;
- a DCC-specific agent;
- a multi-agent framework in v0.1.

RAH owns:

- its public protocol;
- runtime abstractions;
- model abstractions;
- tool abstractions;
- event model;
- session model;
- permission boundaries;
- sandbox abstraction;
- CLI/API boundary.

Codex is initially treated as an optional runtime integration target through `rah-runtime-codex`.

---

## 0.1 Development execution environment

The primary v0.1 development environment is **Codex Desktop operating on a local Git repository**.

```text
Codex Desktop
    -> local Git repository
    -> local Rust/Cargo toolchain
    -> local tests and validation
```

The local Git repository is the source of truth. Codex Desktop is development tooling only and must never become a RAH runtime dependency.

RAH must remain maintainable through ordinary Rust/Cargo/Git workflows so development can later move to Codex CLI, another coding agent, or manual development without architectural changes.

For every numbered task:

1. inspect the working tree before editing;
2. work inside the repository unless explicitly required otherwise;
3. preserve existing user changes;
4. run relevant Cargo checks locally;
5. inspect `git status --short`;
6. inspect the relevant `git diff`;
7. report changed and untracked files.

Unless explicitly instructed, do not push, merge, rebase, reset, change remotes, discard user changes, force-clean the repository, modify unrelated repositories, or change host/system configuration.

If a compiler, toolchain component, or system dependency is unavailable, report the missing prerequisite instead of silently changing the machine.

---

## 1. Architectural invariants

The following rules are mandatory for every task.

### 1.1 Codex isolation

Only `rah-runtime-codex` may directly depend on or import Codex crates.

Forbidden outside that crate:

```rust
use codex_core::...;
use codex_protocol::...;
```

RAH public APIs must not expose Codex types.

### 1.2 Provider isolation

`rah-core`, `rah-agent`, `rah-tools`, and `rah-protocol` must not contain provider-specific branches such as:

```rust
if provider == "openai" { ... }
if provider == "deepseek" { ... }
```

Provider-specific behavior belongs in adapter crates or backend implementations.

### 1.3 No inference engine

RAH must not:

- load model weights;
- implement transformer inference;
- manage CUDA/ROCm/Metal kernels;
- implement KV cache;
- implement tokenizer internals.

Inference is provided by external APIs, local servers, or pluggable backends.

### 1.4 Model output is untrusted

A model request for a tool execution never grants permission by itself.

Required execution path:

```text
Model
  -> ToolCall
  -> ToolRegistry
  -> Policy / Permission
  -> Sandbox / Executor
  -> Tool
  -> Result
```

### 1.5 Stable protocol boundary

Cross-crate messages should prefer neutral RAH types from `rah-protocol`.

### 1.6 Event-driven external interface

Frontends consume `AgentEvent` streams.

CLI/TUI/server code must not depend on runtime internal state.

---

## 2. v0.1 workspace

Create this structure:

```text
rah/
├── Cargo.toml
├── rust-toolchain.toml
├── rustfmt.toml
├── AGENTS.md
├── RAH_IMPLEMENTATION_V0.1.md
├── crates/
│   ├── rah-protocol/
│   ├── rah-core/
│   ├── rah-model/
│   ├── rah-tools/
│   ├── rah-runtime/
│   ├── rah-runtime-codex/
│   ├── rah-session/
│   ├── rah-sandbox/
│   └── rah-cli/
├── tests/
└── docs/
```

Do not create additional crates unless a task explicitly requires it.

---

## 3. Workspace dependency policy

Preferred shared dependencies:

```toml
tokio
serde
serde_json
thiserror
tracing
tracing-subscriber
uuid
chrono
async-trait
futures
clap
reqwest
```

Rules:

- prefer workspace dependencies;
- avoid speculative dependencies;
- do not add a framework merely for future use;
- public library crates should prefer `thiserror`;
- `anyhow` may be used at application boundaries such as CLI code;
- avoid cyclic dependencies;
- avoid feature explosions in v0.1.

---

# Phase A — Foundation

## Task 001 — Bootstrap Cargo workspace

### Goal

Create a compilable Rust workspace with the required crate skeletons.

### Files

- `Cargo.toml`
- `rust-toolchain.toml`
- `rustfmt.toml`
- all crate `Cargo.toml`
- minimal `src/lib.rs`
- `crates/rah-cli/src/main.rs`

### Requirements

Use edition 2024 unless the installed stable Rust toolchain makes this impractical. If impractical, document the reason before choosing edition 2021.

Crate names:

- `rah-protocol`
- `rah-core`
- `rah-model`
- `rah-tools`
- `rah-runtime`
- `rah-runtime-codex`
- `rah-session`
- `rah-sandbox`
- `rah-cli`

### Forbidden

- no Codex dependency yet;
- no network calls;
- no tool implementation;
- no business logic.

### Acceptance

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

All must pass.

### Commit boundary

One commit.

Suggested message:

```text
chore: bootstrap RAH workspace
```

---

## Task 002 — Implement `rah-protocol` identifiers

### Goal

Create strongly typed IDs shared across RAH.

### Public API

Define newtypes:

```rust
SessionId
RequestId
ModelRequestId
ToolCallId
ApprovalId
```

Requirements:

- backed by UUID;
- `Clone`, `Debug`, `Eq`, `PartialEq`, `Hash`;
- serde serializable/deserializable;
- `Display`;
- constructor for new random ID.

### Dependency rules

Allowed:

- `serde`
- `uuid`

Forbidden:

- `tokio`
- `reqwest`
- `codex-*`
- other RAH crates

### Tests

- round-trip serde;
- IDs are distinct;
- display/parse behavior if parsing is exposed.

### Acceptance

```bash
cargo test -p rah-protocol
```

---

## Task 003 — Implement core protocol messages

### Goal

Define provider-neutral, runtime-neutral protocol types.

### Public types

At minimum:

```rust
AgentInput
AgentRequest
AgentOptions
AgentOutput
AgentEvent

Message
MessageRole

ToolName
ToolDefinition
ToolCall
ToolInput
ToolOutput
ToolContent

PermissionLevel
ApprovalRequest

AgentErrorCode
```

### Initial `AgentEvent`

Must include:

```rust
Started
ModelRequestStarted
ModelDelta
ToolRequested
ToolStarted
ToolFinished
ApprovalRequired
Completed
Failed
Cancelled
```

Do not put runtime objects, file handles, channels, or provider SDK types in protocol structs.

### Protocol version

Reserve an explicit initial protocol version:

```rust
pub const RAH_PROTOCOL_VERSION: u32 = 1;
```

Do not implement complex version negotiation yet.

### Serialization

All externally meaningful events must support serde.

### Tests

- serialize/deserialize representative events;
- verify no provider-specific fields are required;
- verify tool calls can contain arbitrary JSON arguments.

### Acceptance

```bash
cargo test -p rah-protocol
```

---

# Phase B — Core abstractions

## Task 004 — Implement `rah-model`

### Goal

Define the model backend abstraction without implementing a real provider.

### Public API

```rust
#[async_trait]
pub trait ModelBackend: Send + Sync {
    async fn complete(
        &self,
        request: ModelRequest,
    ) -> Result<ModelStream, ModelError>;
}
```

Use a concrete stream alias such as:

```rust
pub type ModelStream =
    Pin<Box<dyn Stream<Item = Result<ModelEvent, ModelError>> + Send>>;
```

Define:

```rust
ModelRequest
GenerationOptions
ModelEvent
ModelError
```

`ModelEvent` should support at minimum:

```rust
TextDelta
ToolCall
Usage
Completed
```

Do not model every provider capability yet.

### Forbidden

- OpenAI SDK types;
- Codex types;
- HTTP implementation;
- inference logic.

### Tests

Use a small in-memory fake stream.

---

## Task 005 — Add deterministic `MockBackend`

### Goal

Provide a model backend suitable for deterministic tests.

### Behavior

`MockBackend` accepts a queue of scripted model turns.

Example:

```text
Turn 1 -> ToolCall(fs.read, {"path":"Cargo.toml"})
Turn 2 -> Text("done")
```

It must not require network access.

### Public functionality

- construct from scripted responses;
- inspect number of requests received;
- optionally capture requests for assertions.

### Tests

- scripted event order;
- request capture;
- empty script returns a defined error.

---

## Task 006 — Implement `rah-tools` abstraction

### Goal

Define tools independently from any runtime.

### Public trait

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError>;
}
```

Define:

```rust
ToolContext
ToolError
ToolRegistry
```

### Registry API

At minimum:

```rust
register
get
definitions
execute
```

Registration of duplicate names must fail deterministically.

### Tests

- registration;
- duplicate rejection;
- unknown tool;
- tool execution;
- definitions list.

---

## Task 007 — Add `echo` tool

### Goal

Create the first safe built-in tool.

Name:

```text
echo
```

Input:

```json
{"text":"..."}
```

Output: same text.

Purpose:

- validates schema flow;
- validates registry flow;
- enables agent-loop testing without OS access.

---

# Phase C — Runtime

## Task 008 — Implement runtime abstraction

### Goal

Define the stable RAH runtime interface.

### Public API

```rust
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    async fn start(
        &self,
        request: AgentRequest,
    ) -> Result<AgentHandle, AgentError>;

    async fn resume(
        &self,
        session_id: SessionId,
    ) -> Result<AgentHandle, AgentError>;

    async fn cancel(
        &self,
        session_id: SessionId,
    ) -> Result<(), AgentError>;
}
```

`AgentHandle` must expose:

- `session_id`;
- asynchronous `AgentEvent` stream.

### Important

Do not implement Codex here.

`rah-runtime` must not depend on Codex crates.

---

## Task 009 — Implement minimal test runtime

### Goal

Prove that RAH abstractions work independently of Codex.

This is NOT the future production NativeRuntime.

Create a minimal runtime used for tests/examples that can:

1. send context to `MockBackend`;
2. consume a model tool call;
3. dispatch through `ToolRegistry`;
4. append tool result;
5. invoke the model again;
6. emit RAH events;
7. finish with text.

### Required scenario

```text
User
 -> mock model
 -> echo tool call
 -> echo result
 -> mock model
 -> final answer
```

### Acceptance

An integration test verifies event order.

Expected sequence approximately:

```text
Started
ModelRequestStarted
ToolRequested
ToolStarted
ToolFinished
ModelRequestStarted
ModelDelta
Completed
```

---

# Phase D — Safety and local tools

## Task 010 — Implement sandbox abstraction

### Goal

Define a sandbox API without attempting to implement a complete operating-system sandbox.

### Public API

```rust
#[async_trait]
pub trait Sandbox: Send + Sync {
    async fn execute(
        &self,
        command: CommandSpec,
        policy: SandboxPolicy,
    ) -> Result<ExecutionResult, SandboxError>;
}
```

Define:

```rust
SandboxPolicy
CommandSpec
ExecutionResult
SandboxError
```

Policies:

```text
ReadOnly
WorkspaceWrite
FullAccess
```

Do not claim strong isolation for an implementation that only performs path checks.

---

## Task 011 — Implement workspace filesystem policy

### Goal

Create path validation helpers.

Requirements:

- configure a workspace root;
- resolve relative paths;
- reject escaping workspace root;
- reject `..` escape after canonicalization when applicable;
- clearly handle nonexistent write targets;
- use explicit errors.

Tests must include:

- normal child path;
- `../` escape;
- absolute outside path;
- symlink behavior where supported.

---

## Task 012 — Implement `fs.read`

### Goal

Read UTF-8 text within the configured workspace boundary.

Name:

```text
fs.read
```

Input:

```json
{"path":"relative/path"}
```

Requirements:

- no outside-workspace reads;
- configurable maximum bytes;
- deterministic truncation/error policy;
- binary files return a clear error.

Permission:

```text
Read
```

---

## Task 013 — Implement `shell.exec`

### Goal

Execute a subprocess through the sandbox/executor boundary.

Name:

```text
shell.exec
```

Input should include:

```text
program
args
optional cwd
optional timeout
```

Do NOT make the primary API a single shell string.

Prefer direct process execution.

Requirements:

- capture stdout;
- capture stderr;
- exit status;
- timeout;
- cancellation-ready design;
- workspace cwd validation;
- permission level `Execute`.

Do not implement unrestricted shell interpolation by default.

---

# Phase E — Session and cancellation

## Task 014 — Implement session model

### Goal

Create neutral session state.

Define:

```rust
Session
SessionStatus
AgentContext
```

Statuses:

```text
Running
WaitingApproval
Completed
Cancelled
Failed
```

Context v0.1 only needs:

- messages;
- tool result history;
- metadata.

No vector database.
No long-term memory framework.

---

## Task 015 — Implement `SessionStore`

### Public trait

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn load(
        &self,
        id: SessionId,
    ) -> Result<Option<Session>, SessionStoreError>;

    async fn save(
        &self,
        session: &Session,
    ) -> Result<(), SessionStoreError>;
}
```

Implement:

```text
MemorySessionStore
```

SQLite is explicitly deferred.

---

## Task 016 — Add cancellation propagation

### Goal

Cancellation must propagate from runtime to long-running work.

Use Tokio cancellation primitives.

Required path:

```text
Agent cancellation
 -> model operation
 -> tool execution
 -> subprocess where supported
```

Tests:

- cancel running session;
- receives `Cancelled`;
- no `Completed` after cancellation.

---

# Phase F — CLI

## Task 017 — Implement minimal CLI

### Commands

```bash
rah run "<prompt>"
rah tools
rah doctor
```

`run` initially supports the deterministic/mock runtime configuration used in tests.

Do not add TUI.

### CLI rules

- `clap`;
- errors to stderr;
- nonzero exit code on failure;
- streaming events may print text incrementally;
- debug details are controlled by tracing/log level.

---

## Task 018 — Add tracing

### Goal

Instrument runtime operations.

Use:

```text
tracing
tracing-subscriber
```

Correlation fields should include where applicable:

```text
session_id
request_id
model_request_id
tool_call_id
```

Environment:

```bash
RUST_LOG=rah=debug
```

No custom logging framework.

---

# Phase G — Codex integration spike

## Task 019 — Research current Codex Rust integration boundary

### Goal

Before adding any dependency, inspect the exact checked-out/current Codex API.

Produce:

```text
docs/CODEX_INTEGRATION_SPIKE.md
```

The spike must answer:

1. Which Codex crates are required for starting a headless agent session?
2. Which public APIs appear intended for reuse?
3. Which APIs are internal/unstable?
4. How are sessions started and resumed?
5. How are events represented?
6. How are tool calls represented?
7. How are approvals represented?
8. How is cancellation handled?
9. What sandbox helpers are coupled to Codex assumptions?
10. Can model-provider behavior be replaced cleanly?
11. What transitive dependency cost is introduced?
12. Is direct crate reuse preferable to selective vendoring?

### Hard rule

Do not modify RAH architecture during this task.

If Codex does not map cleanly to an existing RAH boundary, document the mismatch.

---

## Task 020 — Implement `rah-runtime-codex` adapter skeleton

### Goal

Create compilation-level adapter structure only after Task 019.

Only this crate may import Codex crates.

Create translation modules such as:

```text
request.rs
events.rs
tools.rs
errors.rs
runtime.rs
```

No Codex type may cross the crate's public boundary.

---

## Task 021 — Map Codex events to `AgentEvent`

### Goal

Translate Codex runtime events into stable RAH events.

Unknown/new Codex events must not crash the adapter.

Choose one explicit behavior:

- map to a neutral metadata event if available; or
- ignore with tracing; or
- return a defined adapter error when semantically required.

Document the choice.

Tests should use fixtures/mocks when possible rather than live model calls.

---

## Task 022 — Codex conformance test

### Goal

Run the same logical runtime conformance behavior against:

```text
MinimalTestRuntime
CodexRuntime
```

Where live model access would make the test nondeterministic, isolate translation tests and mark external integration tests separately.

The default test suite must not require paid API access.

---

## Task 022A — Establish reusable conformance test helpers

### Goal

Create reusable contract-level test helpers where the current v0.1 abstractions are mature enough.

At minimum, establish deterministic conformance coverage for:

```text
ModelBackend
Tool
SessionStore
AgentRuntime
```

The helpers must test observable RAH behavior, not implementation-private state.

They must not require network access, paid APIs, or a real LLM.

CodexRuntime may use adapter/fixture tests where live upstream execution cannot be deterministic.

---

# Phase H — v0.1 release gate

## Task 023 — Architecture dependency check

### Goal

Verify architectural invariants mechanically where practical.

At minimum:

- search dependencies to ensure only `rah-runtime-codex` references Codex;
- ensure `rah-protocol` has no RAH dependency;
- ensure core crates have no provider SDK dependency.

Document the check in CI or a script.

---

## Task 024 — v0.1 end-to-end test

Target command:

```bash
rah run "read Cargo.toml and report the workspace package information"
```

For deterministic CI, use a scripted/mock model that requests `fs.read`.

Required behavior:

```text
CLI
 -> AgentRuntime
 -> ModelBackend
 -> ToolCall
 -> ToolRegistry
 -> fs.read
 -> ToolOutput
 -> ModelBackend
 -> final answer
```

Verify emitted events and final exit status.

---

## Task 025 — v0.1 documentation

Create/update:

```text
README.md
docs/ARCHITECTURE.md
docs/SECURITY.md
docs/CODEX_INTEGRATION_SPIKE.md
```

README must clearly state:

- RAH is not an inference engine;
- RAH is provider-agnostic;
- Codex integration is optional/adapted;
- v0.1 limitations;
- how to run tests;
- how to run the mock/demo path.

---

# 4. Definition of Done for every task

Before a task is considered complete, Codex must:

1. inspect the relevant existing files;
2. preserve architecture invariants;
3. implement only the current task;
4. add or update tests;
5. run formatting;
6. run targeted tests;
7. run targeted clippy/check where appropriate;
8. report any architectural conflict instead of silently changing boundaries.

Baseline commands:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

For a narrow task, targeted commands may be run first, but the full workspace checks are required before milestone completion.

---

# 5. Change-size policy

Prefer one coherent task per commit.

Unless the task is mechanical:

- aim for <500 changed lines;
- avoid >800 changed lines;
- if a task would exceed this, split it into smaller coherent tasks without changing architecture.

Generated lockfile changes are excluded from conceptual change-size judgment.

---

# 6. Codex behavior when requirements are ambiguous

Codex must NOT silently invent a new subsystem.

Use this order:

1. infer from `AGENTS.md`;
2. infer from this plan;
3. inspect existing RAH public abstractions;
4. choose the smallest implementation that preserves current boundaries;
5. if two choices materially change architecture, stop and report the conflict.

Examples requiring a stop/report:

- moving Codex types into `rah-core`;
- adding a provider-specific field to `AgentEvent`;
- bypassing the sandbox for convenience;
- adding an inference engine;
- introducing a database before the plan calls for it;
- changing public traits merely to match Codex internals.

---

# Architecture guardrails

Before implementation, also read:

```text
docs/ARCHITECTURE_GUARDRAILS.md
docs/adr/
```

The crate dependency matrix, public API stability classes, protocol-version reservation, and conformance-test rules are part of the v0.1 implementation contract.

Material changes to architecture-defining interfaces such as `AgentRuntime`, `ModelBackend`, `Tool`, `AgentEvent`, `SessionStore`, or `Sandbox` require a conflict report before implementation when they are not already authorized by the current task.

Accepted ADRs must not be silently overridden.

---

# Plugin extension point constraint

RAH v0.1 must preserve a future plugin extension point without implementing a full plugin system.

Architecture requirement:

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

All external tool providers must converge into the same RAH-owned `Tool` abstraction.

During v0.1:

- do not implement `PluginManager`;
- do not implement plugin discovery or installation;
- do not load Rust dynamic libraries;
- do not create a plugin marketplace;
- do not expose AgentRuntime internals to plugins.

However, v0.1 implementations of `Tool`, `ToolDefinition`, `ToolRegistry`, permission checks, and AgentRuntime must not assume that all tools are compiled into RAH.

Future plugin transports should prefer process isolation or MCP over Rust ABI-coupled dynamic libraries.

Reserved post-v0.1 roadmap:

```text
Task 026 PluginManifest
Task 027 PluginProtocol
Task 028 PluginProcess
Task 029 PluginToolProxy
Task 030 PluginManager
Task 031 Plugin permissions
Task 032 Plugin lifecycle
Task 033 Plugin conformance tests
```

These are reserved extension points only. Do not implement them unless explicitly instructed.

---

# 7. Explicit v0.1 non-goals

Do not implement during this plan:

- custom inference;
- CUDA/ROCm/Metal execution;
- vector database;
- RAG framework;
- autonomous multi-agent orchestration;
- plugin marketplace;
- general plugin platform;
- TUI;
- web UI;
- distributed scheduler;
- remote worker fleet;
- DCC/Maya/Houdini integration;
- DeepSeek-specific code;
- provider-specific logic in core;
- long-term memory;
- self-modifying architecture.

---

# 8. v0.1 success criteria

RAH v0.1 is successful when all are true:

- `RAH Core` has no Codex dependency;
- `RAH Core` has no DeepSeek dependency;
- `RAH Core` has no OpenAI dependency;
- a deterministic mock model can drive a complete tool loop;
- tools execute through registry + permission/sandbox boundaries;
- sessions can be represented and stored in memory;
- cancellation is represented and tested;
- CLI can run a complete deterministic agent task;
- Codex integration is isolated behind `rah-runtime-codex`;
- Codex can be removed without changing RAH public protocol;
- workspace tests and clippy pass.

---

# 9. Recommended Codex execution prompt

Use this at the start of implementation:

```text
Read and follow the repository root AGENTS.md and
RAH_IMPLEMENTATION_V0.1.md.

Work on exactly one numbered task at a time.

Before editing:
1. inspect the files relevant to the current task;
2. state which architectural invariants apply;
3. identify the smallest coherent implementation.

During implementation:
- do not perform work from later tasks;
- do not alter architecture to fit Codex internals;
- do not expose provider- or Codex-specific types through RAH public APIs;
- add tests for behavioral changes.

After implementation:
1. run targeted tests;
2. run cargo fmt --check;
3. run cargo check/clippy as applicable;
4. summarize changed files, tests, and any unresolved risks.

If the task conflicts with the architecture, stop and report the conflict.
Do not silently reinterpret the architecture.
```

---

# 10. First command to Codex

Start with:

```text
Implement Task 001 from RAH_IMPLEMENTATION_V0.1.md only.

Do not begin Task 002.
Follow AGENTS.md.
After completion, report:
- files created/changed;
- commands run;
- test results;
- any architecture questions.
```
