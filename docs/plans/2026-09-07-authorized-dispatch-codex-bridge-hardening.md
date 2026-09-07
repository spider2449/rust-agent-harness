# Task 238 — Authorized Dispatch Equivalence and Codex Bridge Hardening

## Status

IMPLEMENTED — AWAITING EXACT-HEAD CI

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`.
- `HEAD == origin/master == 5a2c02cabccb70d219f6a1513764cd784b197e17`.
- Worktree clean before implementation.
- Task 237 foundation: `db656fb89c80398ae15c3fd6fd3ff0a14ef91799`.
- Task 237 corrective exact-head CI: `34074833779` passed.

## ADR 0021 / Task 237 baseline

ADR 0021 D2 requires current registry lookup, exact complete `ToolDefinition`
identity, and explicit current permission membership before execution. Task
237 established the neutral authorized dispatch primitive and distinct
dispatch rejection versus underlying `ToolError` semantics.

## Existing Codex bridge admission

The bridge retains Codex-owned namespace, active thread/turn, private alias,
call-ID replay, waiter, cancellation, and terminal handling. It publishes
`ToolRequested` after route ownership and performs lifecycle accounting only
for actual runtime/model dispatch.

## Intended permission-snapshot tightening

The captured definition is now compared in full, including permission. A
current permission change is a `DefinitionMismatch` even when the new
permission is present in `allowed_permissions`; a changed permission cannot
rescue a stale snapshot.

## Shared neutral admission helper

`authorize_tool_dispatch` performs the D2 pre-dispatch checks without executing
or reserving a Tool. `authorized_tool_dispatch` reuses that implementation and
then performs the single registry execution.

## Pre-start lifecycle preservation

The bridge performs neutral admission after `ToolRequested` and before
`ToolStarted`. A rejection completes the tracked call, emits no `ToolStarted`
or `ToolFinished`, executes zero times, and returns the established sanitized
permission-denial response.

## Execution-time D2 revalidation

After `ToolStarted`, one owned task calls `authorized_tool_dispatch` with the
captured definition, the same permission policy, the same call, and default
`ToolContext`. This is a second admission check, not a second execution.

## Late rejection handling

A late D2 rejection is classified as sanitized permission/dispatch denial,
emits `Failed` rather than `ToolFinished`, and becomes terminal without retry.
Definition details are retained only in internal structured tracing.

## Underlying Tool error handling

An admitted underlying `ToolError` remains an `AgentErrorCode::Tool` failure.
It executes once, emits no successful `ToolFinished`, and uses the established
sanitized Codex response.

## Replay / dedup preservation

The existing `(thread_id, turn_id, call_id)` bridge cache remains the only
replay cache. Exact duplicates reuse in-flight or completed results;
conflicting reuse fails closed; completed calls do not execute again.

## Cancellation preservation

Existing owned-task cancellation, waiter rejection, terminal reconciliation,
and no-replay behavior remain unchanged. D2 adds no rollback or retry claim.

## Deterministic equivalence tests

Focused `rah-tools` tests cover exact admission without execution, permission
denial, full-definition mismatch, and reuse of shared rejection semantics.
Bridge coverage preserves normal Requested/Started/Finished and one execution,
permission-denied lifecycle, ToolOutput error semantics, ToolError mapping,
replay/dedup, and cancellation behavior.

## Security hardening tests

Bridge coverage verifies stale permission changes fail closed even when the new
permission is allowed, in both Read-to-Execute and Execute-to-Read directions.
Changed descriptions and schemas remain stale-definition denials with zero
Tool execution.

## No Desktop / protocol expansion

No Desktop, protocol, AgentEvent, Cargo, or dependency changes are included.
No live Codex execution is performed. Task 239 remains separate and is not
started.

## Validation

Required validation was run sequentially:

- `cargo fmt --check`: pass.
- `cargo check --workspace`: pass.
- `cargo test -p rah-tools authorized_dispatch -- --nocapture`: 15 passed.
- `cargo test -p rah-tools`: 192 unit tests passed; integration/doc tests
  passed with 2 documented ignored host-configuration tests.
- `cargo test -p rah-runtime-codex bridge -- --nocapture`: 54 passed.
- `cargo test -p rah-runtime-codex`: 83 passed, 1 documented ignored.
- `cargo test --workspace`: all deterministic tests passed; documented
  host-only/live tests remained ignored.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  pass.
- `git diff --check`: pass.
- `cargo metadata --no-deps --format-version 1`: 13 packages, all `0.19.0`,
  edition `2024`.

## Files changed

- `crates/rah-tools/src/authorized_dispatch.rs`
- `crates/rah-tools/src/lib.rs`
- `crates/rah-runtime-codex/src/bridge.rs`
- `crates/rah-runtime-codex/src/bridge_tests.rs`
- `docs/plans/2026-09-07-authorized-dispatch-codex-bridge-hardening.md`

## Commit

Preferred commit: `feat: harden Codex bridge with authorized dispatch`.

## Exact-head CI

Await the push CI for the exact Task 238 commit on `master` and require
completed/success before marking this task complete.

## Next task

Task 239 — Desktop Explicit Host Tool Invocation Workflow. Not started.
