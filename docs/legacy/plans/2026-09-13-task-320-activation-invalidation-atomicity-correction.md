# Task 320 — RAH v0.26 Repository Activation Invalidation Atomicity Correction

## Authoritative checkpoint

- Current master / Task 319 audit commit: `932d5decb966e6f45196a24aaf5f306579ad628b`
- Direct parent: `e71054b849f941cb71e585041e48b69c39139f87`
- Task 319 exact-head CI: `34732700221` — PASS
- Task 319 verdict: Verdict B — CORRECTION REQUIRED
- Finding: F-319-1 — activation invalidation is not atomic with successful publication
- Accepted architecture: ADR 0027
- Published release: RAH v0.25.0, immutable source `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, annotated tag object `ea3c31aaf5190b632d7ef86387f7aff6004ae664`, GitHub Release `387406579`

## Scope and authority

Correct only activation invalidation ordering and its deterministic regression
coverage. Preserve active-only composition, Task 317 lifecycle exclusion,
Task 315 nested-boundary behavior, the exact 11-name HostExplicit set, public
Tool/provider contracts, and v0.25 version metadata. Do not add persistence,
removal, multi-repository execution, connected switching, frontend scope,
release work, or ADR 0027 changes.

## Defect and ordering extension

The old sequence revoked the active repository's Commit capability and reset
workflow before the final membership/currentness/lifecycle/identity gate.
HostPrepared was also cleared before final target identity revalidation. A
stale target or losing concurrent activation could therefore mutate the
repository that remained active.

The correction separates:

1. Candidate preparation: capture A/B currentness, validate lifecycle and B,
   and construct the fresh B repository without mutating active state.
2. Final validation/reservation under membership and lifecycle exclusion:
   recheck member binding, expected A/generation, lifecycle, B identity, and
   synchronously reserve/revoke the old Commit authorization.
3. Synchronous commit point: invalidate HostPrepared, remove the old Desktop
   Commit capability, reset workflow, and publish active member, repository,
   generation, persistence namespace, and fresh conversation together.

After the first destructive invalidation, no ordinary recoverable validation
remains. A losing activation fails before the commit point and cannot mutate
the winner.

## Commit revocation design

Add a narrow host-only `try_clear_authorization_now` seam to
`RepositoryCommitControl`. It uses the existing short-held pending lock and
returns busy without changing authorization when it cannot acquire it. No
authorization contents, generic Commit authority, Tool schema, serialization,
or model-visible control is added. Existing async authorization and one-shot
execute semantics remain unchanged. The final activation gate attempts this
seam before capability removal; failure leaves the old active state intact.

Final lock order: `membership_coordination`, `lifecycle_coordination`, then
host state locks in the existing synchronous order (`workspace_membership`,
`chat`, `connection`, `host_invocation`, Commit capability/workflow,
repository/generation, persistence, and conversation as needed). No blocking
guard is held across an await; the activation commit point is synchronous.

## Deterministic evidence

Add coverage for stale-after-second-revalidation with A workflow, Stage/
Unstage/review/Commit state, pending authorization, HostPrepared, conversation,
active member/root/generation, and persistence namespace preserved. Add a
loser-after-winner synchronization in which C is prepared, B publishes and has
non-default state installed, then C reaches its final gate and fails without
clearing B workflow, actions, Commit state, HostPrepared, conversation, or
persistence namespace. Retain Confirm, Connect, ModelTurn, HostRunning,
target-removal, currentness, already-active, selector/IPC, nested-boundary, and
exact HostExplicit regressions.

## Validation and closure record

Completed before commit:

- `cargo fmt --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace` — PASS: Desktop 232 passed / 10 ignored;
  `rah-tools` 286 passed; remaining workspace tests and doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `cargo build -p rah-desktop --release` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages,
  version `0.25.0`, edition `2024`; `Cargo.lock` diff is empty.
- Frontend/Tauri — PASS: Node syntax, repository membership, effective
  authority, and Tauri permission tests.
- Focused activation — PASS: 3 Task 320 tests; existing Task 317 activation
  race set remains green.
- Focused Commit — PASS: 16 repository Commit tests, including nonblocking
  revocation-lock busy behavior.
- Task 318 selector/active-only test — PASS: 1.
- Task 315 nested-boundary regressions — PASS: 5 focused `rah-tools` tests.
- `git diff --check` — PASS.

Changed production/test files are `crates/rah-desktop/src/main.rs` and
`crates/rah-tools/src/repository_commit.rs`; this plan is the only
documentation file. Commit subjects, pushed exact head, exact-head CI, and
production commit `93cd67e` (`fix: make repository switch invalidation
atomic`) and the documentation commit are recorded at closure. Exact-head CI
and final Outcome A/B are recorded after publication. Explicit deferrals remain
persistence, removal, live certification, release work, and Task 321 re-audit.

## Closure evidence

The corrected ordering is: candidate capture and construction; final member,
active-generation, lifecycle, connection, HostInvocation, and fresh target
identity validation under `membership_coordination` then
`lifecycle_coordination`; nonblocking Commit revocation reservation; then the
synchronous commit point. The commit point clears HostPrepared, removes the
old Desktop Commit capability, resets the complete workflow, publishes active
member/repository/generation coherently, selects the new persistence namespace,
and starts one fresh conversation. There is no normal fallible check after
invalidation begins.

- Stale B after the second preparation check: A active member/root/generation,
  workflow, Stage/Unstage actions, review, Commit capability and pending
  authorization, HostPrepared ticket, conversation epoch/history, and
  persistence namespace all preserved.
- Loser C after B wins: B generation remained at its single winning increment;
  B workflow/actions, Commit capability/authorization, HostPrepared state,
  conversation, and persistence namespace were unchanged; C returned
  `RepositoryBusy` before the commit point.
- Successful switch: old pending Commit authorization and capability were
  revoked, workflow reset, HostPrepared invalidated, and B published once.
- Confirm winning, Connect, ModelTurn, and HostRunning races remained closed;
  target removal, `.git` replacement, old-member/generation mismatch, and the
  already-active zero-churn path remained closed.
- Task 318 selector/IPC routing remained active-only; HostExplicit eligibility
  remained exactly 11 names; Task 315 nested-boundary regressions remained
  green. No persistence/removal/product/release scope was added.

Production commits are `93cd67e` and `82b64b3`; documentation commits are
`d4b9e76` and `71695f1`. The final pushed head is
`82b64b3944e0d794779160e2708e7f0c8881e769` and passed exact-head CI run
`34735460593`. The closure-document revision was pushed separately and
will receive its own exact-head CI verification. Final result: **Outcome A —
ACTIVATION INVALIDATION ATOMICITY CLOSED**. F-319-1 is closed, including the
HostPrepared final-target-validation ordering extension.
