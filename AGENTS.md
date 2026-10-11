# AGENTS.md — RAH Repository Instructions

## Project identity

RAH (Rust Agent Harness) is a plugin-kernel agent harness written in Rust:
**everything is a plugin**. A minimal kernel owns plugin lifecycle, typed
services, hooks, events, scoped resources and configuration. Model providers,
the agent loop, tools, session logs, storage, sandboxes, policy, context,
skills, scheduling and frontends are all plugins.

RAH orchestrates inference providers. It is not an inference engine. Do not add
model-weight loading, tokenizer or cache internals, hardware inference kernels,
or other inference-engine work (ADR 0004).

The previous authority-first architecture is preserved at tag `v0.35.0` and
branch `legacy/authority-first`. Its instructions are archived in
`docs/legacy/AGENTS.authority-first.md` and do not govern `master`.

## Instruction and source-of-truth precedence

Apply sources in this order:

1. Explicit human or task instruction.
2. Applicable `AGENTS.md` instruction.
3. Accepted ADRs, starting with ADR 0036, plus the current
   `docs/ARCHITECTURE.md` and `docs/SECURITY.md`.
4. The pivot plan (`docs/plans/2026-10-11-plugin-kernel-pivot-plan.md`) and
   other task plans, for phase scope and acceptance criteria.
5. Source and tests, for observable current behavior.

ADRs marked *Superseded* or *Legacy* describe the legacy line. Do not cite them
as constraints on `master` unless a plugin explicitly adopts their rules.
`docs/legacy/` is historical record. Do not edit it except to fix links.

## Architecture rules

- **Kernel stays minimal.** `rah-kernel` contains no agent domain logic, so no
  model, tool, session or policy types. Target size is under 2k lines,
  excluding tests. Lifecycle, services, hooks, events, scopes and configuration
  only.
- **Plugins couple only through services.** Plugins depend on `rah-api` and
  `rah-protocol` only, never on another plugin crate. If two plugins need to
  share something, it belongs in a service contract in `rah-api`.
- **Replace, don't patch.** A new behavior is a new plugin or a hook, not a
  special case in the kernel or in another plugin.
- `rah-protocol` stays dependency-bottom and data-only.
- Keep provider-specific wire formats inside the provider plugin. Service
  contracts in `rah-api` are provider-neutral.
- Plugin forms are static Rust, process (JSON-RPC 2.0 over stdio) and, later,
  WASM. Do not add dylib or `abi_stable` loading.
- Do not introduce a new crate dependency edge without a task-owned reason.
  Architecture tests enforce the `plugins/* -> rah-api, rah-protocol` rule once
  those crates exist.

Before changing the kernel, a service contract in `rah-api` or the plugin
protocol, read ADR 0036 and the pivot plan. If requested work conflicts with
them, stop and report the conflict unless the task authorizes an architecture
change, in which case write or update an ADR.

## Trust and safety invariants

**Mounting is trust; model output is never trusted.**

- Plugins mounted by the user's configuration are trusted code. Model output,
  tool-call arguments and provider metadata are untrusted requests.
- Every model-originated tool call goes through the kernel-level `ToolInvoker`
  and its `tool.before_invoke` hook chain. Do not add paths that let a loop or
  model plugin call tool providers directly.
- A process plugin may call only the services in its own `requires`. The host
  enforces this.
- The `default` preset mounts `policy-authority`. Under it, a missing permission
  assignment fails closed, and provider or tool metadata cannot grant
  permission. The `minimal` preset without policy must warn loudly at startup.
- **Never automatically replay uncertain external effects.** Timeout,
  cancellation, disconnect, crash or a lost response does not imply rollback.
  Session replay re-reads the log and never re-executes tool calls. Record
  uncertain effects as `uncertain`.
- Process supervision is not OS sandboxing. Do not claim network isolation,
  rollback or strong sandboxing unless it is implemented and tested.
- Shell and process tools are opt-in plugins that require a `sandbox` service.
  Use a fixed program plus an argument vector, never shell interpolation, with
  explicit cwd and environment policy, bounded output, timeout and cancellation.
- Do not change global process state to select context. In particular, never
  call `std::env::set_current_dir`. Working directories are per-session
  configuration.

## Lifecycle and async quality

- Every spawned task, child process and connection belongs to a plugin
  `Scope` and is cancelled or released when the scope is disposed. No detached
  tasks without an owner.
- Design long-running async work for cancellation. Do not block async workers
  with long synchronous work, and do not hold mutex guards across `.await`
  without reason.
- No hidden global mutable state. Plugin state lives in the plugin's scope.
- Use idiomatic stable Rust with narrow public APIs and explicit domain types.
  Libraries return typed, structured errors. Avoid `unwrap()` and `expect()` in
  production paths.

## Scope and worktree discipline

Before editing, run:

```powershell
git status --short
```

Inspect and preserve existing user changes. Work only on the requested task or
plan phase, and prefer the smallest coherent patch. Do not implement later
phases preemptively, and do not mix unrelated refactors with features.

Unless explicitly requested, do not:

- discard unrelated user work or use destructive Git operations as shortcuts;
- force-clean, reset, rebase, merge, change remotes, or modify unrelated
  repositories;
- modify host configuration or install system-wide dependencies;
- touch `.vscode/`.

Legacy crates (`rah-desktop`, `rah-runtime`, `rah-model`, `rah-session`,
`rah-profile-composition`, `rah-tools-plugin`, `rah-runtime-openai`) may receive
only the minimal changes needed to keep the workspace building, until their
pivot phase removes or transforms them.

On Windows, use native PowerShell-safe commands. Avoid placeholder syntax such
as `<file>` in commands meant for direct pasting, and quote Windows paths
explicitly when needed.

## Testing and validation

Behavior changes require deterministic tests. Normal workspace tests must not
require paid credentials, a live model, internet access or GPU hardware, unless
the task explicitly authorizes live validation. Kernel changes require
lifecycle tests: mount and unmount order, dependency loss and recovery, scope
disposal and hook semantics.

At phase or release boundaries, run:

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

Do not claim validation passed unless the commands were actually executed.
Report known failing tests explicitly.

## Commit and completion discipline

One task normally maps to one coherent commit. Do not commit unless the task or
user authorizes it. Before closure, inspect:

```powershell
git status --short
git diff --stat
git diff --check
```

When the workflow requires push and CI closure, push normally, verify that local
`HEAD` equals the origin branch, and require successful CI for that exact head
before starting the next task.

Completion reports state the summary, changed files, validation, ADR and
plugin-contract impact, Git state, remaining risks, and the suggested next
task.
