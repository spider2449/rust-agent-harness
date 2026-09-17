# Task 349 — Zero-Active Repository Withdrawal Foundation

Status: implementation complete; local gates and exact-head CI pass.

Parent checkpoint: `a5b896e6a6184c0aff66acb6f1e9a3d9b22aefbf`.
Authoritative contract: [Task 348 active repository close contract](2026-09-16-task-348-active-repository-close-contract.md).

## Implementation scope

Implement the Task 348 Close transition as one backend lifecycle path. Add a
closed request with `expectedActiveMemberId` and
`expectedRepositoryGeneration`, a narrow Tauri command, sanitized success and
error values, and a membership primitive that clears only the active member.
Retain every member and leave `membership_generation` unchanged. Keep HostExplicit
at eleven tools and register only the exact command permission; do not add the
permission to the default capability.

Use the established `membership_coordination` then
`lifecycle_coordination` order. Require the current expected member, coherent
`DesktopRepository`, matching positive repository generation,
`NotConnected`, no provider activation, no model/chat owner, no
HostRunning/index reservation, and a synchronously clearable Commit control.
Perform no Git, filesystem, provider, runtime, event, or persistence work in
Close. Clear safe HostPrepared, Commit, workflow, and repository state; advance
the existing repository generation once with checked arithmetic; call
`DesktopConversationState::start_new()` once; then publish no active member.

## Currentness and publication fixes

- Use one checked repository-generation advance helper for Close and activation
  or switch publication. Exhaustion returns a bounded unavailable result
  without mutation or wraparound.
- Treat the member selector and repository generation only as currentness
  guards. Reject malformed, unknown, zero, stale, changed-active, and
  structurally inconsistent states without repairing them.
- Prepare workflow selectors and file observations outside lifecycle
  coordination. At final refresh publication, acquire the lifecycle gate,
  recheck repository generation, the exact current repository Arc, and the
  captured active member, then install only in-memory workflow state.
- Retain the disconnected runtime/provider requirement and distinct busy
  outcomes for model ownership, repository effects, and runtime/provider state.

## Deterministic test matrix

- Membership active-to-none retention, unchanged member identity/order and
  membership generation, repeated Close, and explicit reactivation.
- Closed request deserialization, unknown-field rejection, sanitized errors
  and success, correct/stale/unknown guards, A-to-B intent binding, and
  generation exhaustion for Close and activation.
- Runtime/provider, model/chat, HostRunning, HostPrepared, Stage/Unstage
  reservation, and Commit authorization ownership.
- Zero-active coherence, one-time conversation reset, retained members,
  stale workflow selectors after reactivation, transcript/catalog byte
  preservation, and disconnected reactivation.
- Barrier-controlled Close versus activation, Commit authorization
  publication, HostExplicit final preparation publication, and the late
  repository-workflow refresh publication race. Connect and HostExplicit
  Confirm orderings are exercised through their lifecycle gates; Stage
  reservation ownership is preserved until its existing completion path.

## Nonclaims and exclusions

No new authority, Tool or model route, ADR, dependency, persistence/schema
change, frontend UX, default-capability permission, implicit Disconnect,
provider/runtime shutdown, rollback, Activity event, transcript mutation,
remembered-catalog mutation, Git operation, or filesystem mutation is part of
Task 349. This backend task does not claim GUI automation or model-selected
Close execution.

## Validation

Required local checks:

```text
cargo fmt --check
cargo check -p rah-desktop
cargo test -p rah-desktop task349_close -- --test-threads=1
cargo test -p rah-desktop -- --test-threads=1
cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings
node crates/rah-desktop/tauri_permission_test.js
git diff --check
```

Results:

- `cargo fmt --check` — PASS.
- `cargo check -p rah-desktop` — PASS.
- `cargo test -p rah-desktop task349_close -- --test-threads=1` — PASS,
  14 tests.
- `cargo test -p rah-desktop -- --test-threads=1` — PASS, 306 passed and
  14 intentionally ignored.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` —
  PASS after scoping two test mutex guards lexically.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.
- `git diff --check` — PASS.
- Exact-head full-workspace CI for the implementation commit on `master`, SHA
  `4a29ef58533a2e9fcc9e16cb28f47b8e74cdafb7`, run `35177852723` attempt 2 —
  PASS (formatting, workspace check, workspace tests, and workspace lint).
  Attempt 1 hit a parallel temporary-root collision in the existing
  `rah-tools` rename test fixture during `git init`; all 42 focused rename
  module tests passed locally, and the same-SHA CI retry passed.

No Cargo manifest, lockfile, ADR, frontend asset, default capability, or
persistence schema changes are included. Task 350 owns frontend enablement and
the user-facing Close workflow.
