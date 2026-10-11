# Task 317 — Activation Publication Currentness Correction

## Status

Implementation in progress. This plan records the focused correction required
after Task 316's reported membership-foundation closure was independently
blocked on an active-repository publication currentness race.

## Authoritative baseline

- Task 316 final master: `e741e14667fda69a7fe18358b90e6c088b397cc0`
- Task 316 production commit: `ad56f0c85bae5a41175e85d37e534f57838d8825`
- Task 316 correction commits: `742739cbf9711030c684b813ed4b612dc8143ebe`, `185db1d24cbcab81823ac76a041df53a87c76f3b`
- Task 316 exact-head CI: `34699652744` (PASS)
- Accepted ADR: ADR 0027

## Scope

Correct only the host-state/lifecycle race in private
`activate_admitted_member`. Do not expose repository switching UX, persistence,
new authority, provider changes, or Tool/public contract changes.

## Transaction model

Activation captures the target member ID, immutable admission generation and
admission identity, expected active member, and expected active repository
generation. It validates and constructs outside the publication section,
withdraws old Commit authorization, then enters a final membership/lifecycle
critical section. The final section re-fetches and revalidates the target,
checks the captured active expectations, excludes connection/model/HostInvocation
transitions, invalidates only HostPrepared state, and publishes active member,
repository, and generation under the same host-owned exclusion.

The lifecycle lock ordering is documented in the implementation: mutation
transitions first acquire `lifecycle_coordination`, then their state locks;
activation acquires `membership_coordination`, then `lifecycle_coordination`,
then performs synchronous final checks/publication. No blocking lock is held
across an await.

## Deterministic evidence

Add test-only pre-publication barriers and deterministic tests for connection,
model-turn, prepared Confirm, HostRunning, concurrent B/C activation,
currentness changes, target removal/staleness, invalidation/revocation,
conversation reset, inactive authority, and unchanged HostExplicit eligibility.
Retain Task 316 and Task 315 nested-boundary suites.

## Validation and delivery

Run the required format, workspace check/test/clippy, metadata, diff, Desktop
release build, frontend syntax/authority, Tauri permission, focused race, and
nested-boundary gates. Then commit the production/docs evidence, push `master`,
and require exact-head CI PASS with `HEAD == origin/master` and a clean
worktree. Task 316 is formally closed only after Outcome A for this correction.

## Implementation notes

The correction uses `DesktopAppState::lifecycle_coordination` as the shared
host lifecycle exclusion. Activation captures its transaction under
`membership_coordination` and this exclusion, performs repository validation
and construction outside the final section, then synchronously re-fetches the
member, checks admission generation and complete private identity binding,
checks the previous active member and repository generation, revalidates Git
identity, gates chat/connection/HostInvocation, invalidates `HostPrepared`, and
publishes active member/repository/generation. Old Commit authorization is
withdrawn before the final gate and is never restored on a failed stale/busy
publication.

Test-only barriers deterministically cover connection, model turn, prepared
Confirm, HostRunning, concurrent B/C activation, target removal and `.git`
replacement, and prior-active-member/generation changes. Existing Task 316
inertness/invalidations and Task 315 nested-boundary coverage remain in scope.

## Pre-commit validation

- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages, version
  `0.25.0`, edition 2024.
- `cargo build -p rah-desktop --release` — PASS.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.
- `cargo test -p rah-tools --test repository_nested_boundary` — PASS.
- Focused activation race/currentness tests — PASS: 11 tests including the
  retained Task 316 activation test and new deterministic barriers.
- Exact `cargo test --workspace` rerun — PASS: Desktop 227 passed/10 ignored,
  `rah-tools` 285 passed, all remaining workspace and doc-test suites passed.
  An earlier run had one existing timing-sensitive `rah-tools` test classify
  a timeout as `failed_known`; its isolated rerun and this exact full rerun
  both passed.

No Cargo manifest or `Cargo.lock` drift was observed. No connected Codex live
effect was run. Filesystem race freedom is not claimed.

## Closure evidence

**Outcome A — ACTIVATION CURRENTNESS CORRECTION CLOSED.**

Production commit: `ad1e9a4361516c7ef984d0ab7eefa172d336cb15` (`fix: make
repository activation publication current`). Documentation follow-up commit:
this closure update. The production parent is Task 316 final docs commit
`e741e14667fda69a7fe18358b90e6c088b397cc0`; Task 316 production and correction
commits remain in the parent chain.

The activation transaction captures target member ID, target admission
generation and complete private admission binding, expected old active member,
and expected active repository generation. Final publication uses the current
catalog member and identity revalidation, rejects target removal/staleness,
rejects a changed old member or generation, and increments the active
repository generation exactly once for the winner. The shared lifecycle
exclusion synchronizes connection, model/chat, and HostInvocation transitions;
`HostPrepared` is invalidated inside the same final critical section, while
`HostRunning` and `ModelTurn` fail closed. A prepared Confirm race and Connect,
model-turn, HostRunning, B/C activation, member-removal, `.git` replacement,
old-member, and generation races were deterministic and passed. Losing
activation leaves the winner's active member/repository/workflow/conversation
untouched. Old Commit authorization is withdrawn before publication and is
never restored; successful activation clears workflow/action/review state and
starts exactly one fresh conversation context.

Task 316's inert-member guarantees remain intact, the exact HostExplicit set is
11, Task 315 nested-boundary regression passes, package metadata remains 13 /
`0.25.0` / edition 2024, and persistence or explicit switching UX remains
absent. The immutable v0.25.0 source/tag identity remains
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` / tag object
`ea3c31aaf5190b632d7ef86387f7aff6004ae664`; GitHub Release `387406579` is
unchanged.

Production-head exact CI: run `34728325462` passed for the production SHA.
The documentation follow-up must receive its own exact-head CI pass before
final publication closure.
