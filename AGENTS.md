# AGENTS.md — RAH Repository Instructions

## Project identity

RAH — Rust Agent Harness — is a Rust agent harness and orchestration system.
It orchestrates inference providers; it is not an inference engine.

RAH-owned abstractions define the public and core boundaries. Runtime and
provider implementations sit behind those abstractions. Codex is an optional
`AgentRuntime` adapter, not the definition of RAH.

Keep public and core layers provider-neutral. Do not move provider-specific
types, control flow, or wire contracts into neutral RAH APIs. Provider
translation belongs at adapter edges.

## Instruction and source-of-truth precedence

Apply sources in this order:

1. Explicit human or task instruction.
2. Applicable `AGENTS.md` instruction.
3. Accepted ADRs and current architecture or security documentation.
4. Task plans, milestone audits, and release gates for task-specific facts.
5. Source and tests for observable current behavior.

Read the relevant authoritative material before changing its subject:

- `README.md` for project orientation and supported behavior.
- `docs/ARCHITECTURE.md` and `docs/ARCHITECTURE_GUARDRAILS.md` for boundaries
  and crate topology.
- `docs/SECURITY.md` for authorization and implementation guarantees.
- `docs/adr/` for accepted architectural and security decisions.
- `CHANGELOG.md`, `docs/plans/`, and release or milestone gates for historical,
  task-specific, and release-specific evidence.

Accepted ADRs are decisions, not suggestions. Do not create new architectural
truth in this file or change detailed documentation to justify an implementation
shortcut.

## Architecture and extension boundaries

RAH is not an inference engine. Do not add model-weight loading, tokenizer or
cache internals, hardware inference kernels, or other inference-engine work.

Do not change public abstractions merely to mirror an upstream provider API.
Keep `rah-protocol` dependency-bottom among RAH crates and business-logic-light.
Do not introduce a new crate dependency edge without a clear task-owned reason.

`Tool` and `ToolRegistry` are the external capability boundary:

```text
Built-in Tool
MCP Provider Tool
Process Plugin Provider Tool
        |
        v
     Tool -> ToolRegistry
```

Built-ins, MCP providers, and Process Plugin providers must adapt into the
same RAH-owned `Tool` abstraction. Providers do not own RAH authorization.
Do not make `AgentRuntime` depend on concrete tool or provider implementations,
and do not add generic provider bypasses around `ToolRegistry` or policy.

Before changing an architecture, extension, security, or authority boundary,
inspect the relevant ADRs and architecture/security documentation. If requested
work would weaken or replace an accepted boundary, stop and report the conflict
unless the task explicitly authorizes architecture research or change.

## Authority and security invariants

**MODEL REQUEST IS NOT AUTHORIZATION.** Model output and provider metadata are
untrusted requests. The trusted host owns authority composition.

- Missing required external permission fails closed.
- Provider metadata cannot grant or escalate permission.
- A Trusted Profile is a host authority-composition boundary, not model
  authority.
- Frontend presentation or control is not authority.
- Execute permission is an applicable dispatch permission only; it does not
  implicitly grant narrower mutation authorities.
- Do not add generic shell or process authority.
- Do not bypass `ToolRegistry`, host permission decisions, or applicable
  capability policy and sandbox boundaries.

Repository authorities remain intentionally separate. Preserve the distinction
between observation/read, bounded worktree mutation, index mutation, and
reviewed commit/history mutation. Do not collapse them into generic filesystem
or Git authority. Consult the accepted ADRs for the detailed mutation and
commit contracts.

Do not automatically replay uncertain external effects. Timeout, cancellation,
disconnect, crash, or a lost response does not imply rollback. Do not claim
network isolation, rollback, or strong OS sandboxing unless separately
implemented and proven. Process supervision is not OS sandboxing.

## Repository context and process safety

Repository selection is host-owned. Model output must not select arbitrary
execution roots. Preserve per-runtime and per-generation repository-context
semantics; do not solve context selection by globally changing process CWD with
`std::env::set_current_dir`.

Filesystem and repository operations must remain bounded to approved workspace
or repository authority. Test traversal, symlink, and Windows reparse behavior
where relevant.

For subprocess capabilities, use a fixed program plus argument vector rather
than shell interpolation. Apply explicit cwd and environment policy, bounded
output, timeout/cancellation, and lifecycle ownership. A spawned process does
not grant generic shell, filesystem, network, or Git authority.

## Scope and worktree discipline

Before editing, run:

```powershell
git status --short
```

Inspect and preserve existing user changes. Work only on the requested task and
prefer the smallest coherent patch. Do not implement later roadmap work
preemptively or mix unrelated refactors, architecture changes, and features.

Unless explicitly requested, do not:

- discard unrelated user work or use destructive Git operations as shortcuts;
- force-clean, reset, rebase, merge, change remotes, or modify unrelated
  repositories;
- modify host configuration or install system-wide dependencies;
- touch `.vscode/`.

On Windows, use native PowerShell-safe commands. Avoid placeholder syntax such
as `<file>` in commands intended for direct pasting, and use explicit quoted
Windows paths when needed.

If architecture or security ambiguity is material, stop and report it rather
than guessing. If live validation exposes a real prerequisite defect, diagnose
the exact layer before broadening scope. Do not turn a fallback-path observation
into a requirement without direct evidence.

## Implementation quality

Use idiomatic stable Rust with narrow public APIs and explicit domain types.
Reusable libraries should preserve typed, structured errors rather than erase
domain information at their boundaries.

Design long-running async work for cancellation. Keep lifecycle ownership
explicit; do not leave detached tasks without ownership, block async workers
with long synchronous work, hold mutex guards across `.await` without reason,
or rely on hidden global mutable state. Avoid unnecessary `unwrap()` and
`expect()` in production paths.

## Testing and validation

Behavior changes require appropriate deterministic tests. Normal workspace
tests must not require paid credentials, a live model, internet access, or GPU
hardware unless the task explicitly authorizes live integration validation.
Prefer observable behavior through RAH-owned interfaces.

Use focused checks for individual tasks. At milestone or release-quality
boundaries, run:

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

Do not claim validation passed unless the commands were actually executed.

## Commit and completion discipline

One task normally maps to one coherent commit. Do not commit unless the task or
user authorizes it. Before closure, inspect:

```powershell
git status --short
git diff --stat
git diff --check
```

When the workflow requires push and CI closure, push normally, verify local
`HEAD` equals the origin branch, and require successful CI for that exact head
before starting the next task.

Completion reports should state the summary, changed files, validation,
dependency/ADR/authority impact, Git state, remaining risks, and suggested next
task.
