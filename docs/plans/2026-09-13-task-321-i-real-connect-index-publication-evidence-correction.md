# Task 321-I — Real Connect / Index Publication Evidence Correction

## Scope

Correct Task 321-H's evidence-fidelity finding only. The correction must drive
Stage and Unstage reservation creation through `begin_repository_index_effect`
and drive final Connected rejection through
`publish_connected_provider_state`. Production authority, public contracts,
providers, persistence, release state, and live certification are out of
scope.

## Authoritative checkpoint

- Current master: `ed52503f7c0441e01f4bade97ac058abca9ec511`
- Direct parent: `18f69a4123f27ef824674d916cb409a6b3f543e2`
- Task 321-H exact-head CI: `34756182679` — PASS
- Task 321-H verdict: Verdict B — correction still required
- Accepted architecture: ADR 0027

## Implementation plan

1. Inspect existing Desktop repository workflow, runtime, provider, and commit
   test fixtures.
2. Add deterministic Windows tests in `crates/rah-desktop/src/main.rs` that:
   - obtain real Stage and Unstage action IDs from refreshed workflow state;
   - install both reservations through `begin_repository_index_effect`;
   - prove binding, token, action consumption, early `RepositoryBusy`, and
     reservation preservation;
   - construct a real `Arc<CodexRuntime>` from a local deterministic app-server
     fixture, build a pending publication with matching five-generation
     currentness, and call `publish_connected_provider_state`;
   - prove `IndexEffectActive` rejects before Connected/provider/Commit
     publication, returns runtime ownership, preserves the reservation, maps
     cleanup to `RepositoryBusy`, completes the reservation normally, and
     permits a fresh Connect admission.
3. Remove the core Task 321-G shortcut evidence while retaining its regression
   coverage through the real-path tests.
4. Verify the requested focused suites and full deterministic gates, with no
   cargo, dependency, lockfile, ADR, frontend, permission, or production
   runtime behavior changes.

## Validation and closure

Run the required formatting, check, workspace test, clippy, diff, metadata,
release build, frontend/permission, focused regression, commit, activation,
membership, and nested-boundary checks. Commit only the test and plan changes,
push `master`, and require clean worktree, `HEAD == origin/master`, and
successful exact-head CI before reporting Outcome A. Do not run or claim
v0.26 live certification.

## Implementation evidence

The correction uses only the Windows test module in `main.rs` and this plan.
The real reservation helper obtains workflow action IDs after normal
observation and calls `begin_repository_index_effect`; it asserts token,
generation, active-member, repository-Arc, kind, action-consumption, review
invalidation, and reservation publication. Stage and Unstage each reject real
`begin_connect` with `RepositoryBusy`, preserve `NotConnected` and the
connection-generation counter, complete through `complete_repository_index_effect`,
and allow a fresh Connect start.

The final publication test enters `Connecting`, increments the captured
connection generation as the production Connect path does, verifies matching
repository/model/profile/connection/identity generations, starts a second real
Stage reservation while Connect is pending, and calls
`publish_connected_provider_state`. It receives `IndexEffectActive`, leaves
Connecting/provider/Commit state unpublished, drops the pending Commit
capability, returns and shuts down the real Codex runtime owner, maps the
rejection to `RepositoryBusy`, preserves and completes the reservation, and
allows a fresh Connect start. The pending provider fixture is empty, so
provider activation cleanup is proven as absent rather than by launching an
external provider. The Codex owner is a local deterministic app-server fixture,
not the certified Codex executable or a live test.

Measured deterministic validation before publication:

- Task 321-I: 3 focused tests passed.
- Task 321 regression filter: 8 passed (Task 321-C/E plus the three real I
  tests).
- Task 320: 3 passed; Task 318: 1 passed; activation family: 14 passed.
- Repository Commit: 18 passed; nested-boundary: 1 passed.
- Frontend status, membership, and Tauri permission checks: passed.
- Workspace: Desktop 241 passed/10 ignored, runtime-codex 83 passed/1
  ignored, rah-tools 288 passed, with all other workspace suites passing.
- `cargo fmt --check`, workspace check, workspace Clippy with warnings denied,
  metadata, release build, and `git diff --check`: passed.
- Metadata: 13 packages, version `0.25.0`, edition 2024; no Cargo.lock or
  dependency drift.

No production runtime behavior, public contract, authority, provider,
permission, frontend, ADR, persistence, removal, release identity, or live
certification state was changed.
