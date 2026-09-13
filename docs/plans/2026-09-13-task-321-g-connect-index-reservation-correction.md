# Task 321-G — RAH v0.26 Connect versus Repository Index-Effect Reservation Correction

## Checkpoint and scope

- Task 321-F audit head: `85ffefb4d7b0dec1e3a6746580a67591b4567eeb`.
- Direct parent: `c93474ae14034ff6164ece7ed636ab6d398ef8a2`.
- Task 321-F exact-head CI: `34750612953` — PASS.
- Task 321-F verdict: Verdict B — CORRECTION STILL REQUIRED.
- Accepted architecture: ADR 0027 — Workspace/Repository Identity and
  Authority-Composition Boundary.
- Published release remains v0.25.0, source
  `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, annotated tag object
  `ea3c31aaf5190b632d7ef86387f7aff6004ae664`, GitHub Release `387406579`.

This task remains limited to the single Connect/index lifecycle gap. It adds
no persistence, member removal, connected repository switching, HostExplicit
authority, provider protocol, Tool/schema, frontend, Tauri permission, Cargo,
ADR, release, or live-certification scope.

## Defect and correction

Task 321-F found that `connect_codex` could move an idle disconnected state to
`Connecting`, prepare a runtime and composition, and let
`publish_connected_provider_state` publish `Connected` while a Stage/Unstage
`RepositoryIndexEffectReservation` was still active.

The correction has two independent host-owned checks in
`crates/rah-desktop/src/main.rs`:

1. `begin_connect` checks the reservation under `lifecycle_coordination`
   before the `NotConnected`/`Error` to `Connecting` transition. It returns the
   existing bounded `RepositoryBusy`, without incrementing the connection
   generation or constructing runtime, registry, provider activation, or
   Commit capability state.
2. `publish_connected_provider_state` checks the live reservation while the
   lifecycle lock is held, before provider activation, Connected state,
   composition, permissions, or Commit capability publication. The private
   `IndexEffectActive` rejection is mapped to `RepositoryBusy` after locks are
   released. Existing rejected-publication cleanup shuts down the pending
   runtime and provider activation, drops the unpublished Commit capability,
   and returns `Connecting` to `NotConnected`.

The final gate is independent of admission: a Stage reservation can be
installed while Connect is paused at `connect_publication_test_hook`; resuming
the publication path observes the reservation. The reservation is not cleared
or cancelled by Connect rejection. Stage and Unstage remain separate index
authorities and retain their existing one-attempt, captured-repository refresh,
token cleanup, and no-replay behavior.

## Deterministic coverage

- Stage and Unstage reservations already active before Connect admission both
  return `RepositoryBusy`; connection remains `NotConnected`, generation and
  reservation ownership remain unchanged, and startup counters prove no
  runtime/provider/registry/Commit construction occurs.
- Stage reservation installed after Connect has reached the pre-publication
  barrier is observed by the shared final-publication gate; no Connected state,
  provider owner, or Commit capability is installed and the reservation remains
  active.
- Unstage reservation is independently observed by the same final gate.
- Existing Task 321-C writer coordination, Task 321-E identity/reservation
  tests, Task 320 activation atomicity, Task 318 selector/switch, Task 315
  nested-boundary, Commit, Stage, Unstage, and HostExplicit regressions remain
  covered by the full validation run.

The current product behavior permits Stage/Unstage according to its existing
connected-state policy. This task does not add a disconnected-only rule or
change that policy; lifecycle reservation still blocks the Task 321-E writers
and activation paths.

## Lock order and ownership

Connect admission and final publication acquire `lifecycle_coordination` before
reservation/currentness state access. The final check is synchronous. Runtime
and provider shutdown occur only after the publication lock is released. No
blocking guard crosses an await, and no HostInvocationCoordinator reservation
is used for Stage/Unstage.

## Validation

Passed before publication:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace -- --test-threads=1`: Desktop 241 passed/10 ignored;
  rah-runtime-codex 83 passed/1 ignored; rah-tools 288 passed; all other
  workspace, integration, and doc-test suites passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`: 13 packages, version 0.25.0,
  edition 2024.
- `cargo build -p rah-desktop --release`
- `node crates/rah-desktop/frontend/status_authority_test.js`
- `node crates/rah-desktop/frontend/repository_membership_test.js`
- `node crates/rah-desktop/tauri_permission_test.js`
- `cargo test -p rah-desktop task_321 -- --nocapture --test-threads=1`: 8
  passed, 0 failed, 243 filtered.

No live certification was run. The full suite retained the existing ignored
host/live probes and does not claim live Codex, native-effect, race-free TOCTOU,
rollback, crash recovery, OS sandboxing, network isolation, or cross-platform
live parity.

## Closure record

- Production commit: pending.
- Documentation/test commit: pending.
- Exact-head CI: pending.
- Final HEAD/origin alignment and clean worktree: pending.
- Deferrals: Task 321-H independent audit remains next; Task 322 remains
  blocked until that audit produces the required Verdict A. No release or
  publication work is part of this task.
