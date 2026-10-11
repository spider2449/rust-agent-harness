# Task 321-C — Writer Coordination Correction

## Authoritative checkpoint

- Task 321 head: `eff11d72ebe8856cb8e6898ff182272814ad1e6b`
- Direct parent: `29bdb4d9d7dd286871de7e4b274c794ac01b71d6`
- Task 321 exact-head CI: `34736823070` — PASS
- Task 321 verdict: Verdict B — CORRECTION STILL REQUIRED
- Task 320 outcome: Outcome A — ACTIVATION INVALIDATION ATOMICITY CLOSED
- Accepted architecture: ADR 0027
- Published release: RAH v0.25.0, source `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`

## Scope

Correct the independent writer-coordination defect identified by Task 321.
Activation remains the sole repository switching path. This task adds no
persistence, member removal, workspace-wide authority, Tool authority,
connected repository migration, frontend product scope, permission changes,
provider protocol changes, or release metadata.

## Defect and correction

The unsafe authorization sequence was:

```text
A has reviewed Commit state
-> authorization captures A control/review
-> B activation successfully reserves and clears A authorization
-> old authorization resumes and arms A again
-> old writer may write AuthorizedPending into B's workflow
```

Reviewed Commit authorization is corrected to host-only preparation and final
publication. Capture and supersession clearing happen under the short
`lifecycle_coordination` gate. Async review validation produces an opaque,
non-serializable, one-shot candidate without changing the shared pending slot.
Final publication reacquires the lifecycle gate and checks repository, model,
identity, workflow observation/selector, and exact `Arc` capability identity
before synchronously installing the candidate and setting `AuthorizedPending`.

`set_model_configuration`, `reset_model_preferences`, and `set_commit_identity`
take the same lifecycle exclusion, revoke the shared pending authorization
before invalidating Commit/workflow state, and retain the existing preference
behavior. Disconnect withdraws the Desktop capability and workflow under the
lifecycle gate, then completes any necessary async pending-slot clear after
the gate is released. No blocking guard crosses an await.

The lock order for modified synchronous transitions is
`lifecycle_coordination` → `preference_ordering` → authoritative state locks;
activation retains `membership_coordination` → `lifecycle_coordination` and
its existing host-state order.

## Deterministic coverage

Add barrier-driven tests for authorization preparation versus activation,
model/reset/identity changes, Disconnect, reverse winner order, pending-slot
busy behavior, and stale workflow protection. Retain Task 320 atomicity,
Task 317 races, Task 318 active-only switching, Task 315 nested-boundary, and
the exact 11-name HostExplicit set.

## Validation and closure

Production commit: `f1602ac` (`fix: serialize repository authority writers`).
The production parent is the Task 321 head `eff11d72ebe8856cb8e6898ff182272814ad1e6b`.

The corrected authorization path is:

1. Under `lifecycle_coordination`, capture repository/model/identity and
   observation generations, the review selector, and the exact Commit control;
   synchronously supersede the old pending approval.
2. Outside blocking lifecycle locks, asynchronously prepare an opaque,
   host-only candidate. Preparation does not touch the shared pending slot.
3. Under `lifecycle_coordination`, revalidate every captured generation,
   workflow observation/selector, connection state, and `Arc` control identity.
   The one-shot candidate is synchronously installed into the existing single
   pending slot, then `AuthorizedPending` is published under the same gate.
   A stale or busy publication does neither.

`PreparedRepositoryCommitAuthorization` and
`PreparedRepositoryCommitAuthorizationError` are the only new `rah-tools`
host-side API. Candidate fields are private; the type has no serialization,
permission level, provider access, Tool implementation, or model surface. A
wrong control/policy is rejected, a busy slot is nonblocking, and every
candidate is consumed exactly once.

Model configuration, model reset, and Commit identity updates now take
`lifecycle_coordination` before `preference_ordering`, revoke the shared
pending slot before invalidating Commit capability/workflow state, and retain
existing preference-save behavior. Disconnect marks the connection as
disconnecting, withdraws capability/workflow under the lifecycle gate, and
only then performs any required asynchronous slot clear after releasing the
blocking guard. The modified lock order is
`membership_coordination -> lifecycle_coordination -> preference_ordering ->
authoritative state locks`; no blocking guard crosses `.await`.

Deterministic coverage includes the paused authorization-preparation versus B
activation race, stale A rejection, unchanged B workflow/capability, pending
slot busy/no-partial-publication, exact-control binding, one-shot semantics,
and the retained Task 320 stale-target/loser/successful-revocation tests.
The full Desktop suite retains Task 317 races, Task 318 selector/switching,
active-only membership, Task 315 nested-boundary, Disconnect, and exact
HostExplicit eligibility coverage.

Final local validation:

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test --workspace`: PASS; Desktop `233 passed, 10 ignored`,
  `rah-tools 288 passed`, with all other workspace, integration, and doc tests
  passing.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS.
- `cargo build -p rah-desktop --release`: PASS.
- Frontend syntax, membership, effective-authority, and Tauri permission
  tests: PASS.
- `git diff --check`: PASS.

No Cargo manifest, dependency, or lockfile change; no frontend or permission
change; package count remains 13, version `0.25.0`, edition 2024. Persistence,
member removal, connected repository migration, workspace-wide authority,
release/version changes, live certification, and Task 322 remain deferred.
Exact-head CI, final Git alignment, and the final Outcome A/B are recorded in
the closure report after publication.

## Final closure report

Outcome A — WRITER COORDINATION CORRECTION CLOSED.

- Production commit: `f1602ac`; documentation commits: `fdc24c2`, `ed328a4`.
- Parent chain: `eff11d7 -> f1602ac -> fdc24c2`.
- The authorization-loses case rejects stale A by generation, workflow, and
  exact capability identity; it cannot re-arm the old pending slot or write
  B workflow state. The authorization-wins case publishes coherently and is
  revoked by the normal subsequent activation gate.
- Model configuration, model reset, Commit identity, and Disconnect all share
  the lifecycle ordering with activation. Commit revocation precedes capability
  and workflow withdrawal; busy revocation has no partial writer publication.
- Task 320 stale-target, loser-after-winner, and successful-switch atomicity
  remain passing. Task 317 activation races, Task 318 switching/selector,
  Task 315 nested boundaries, and HostExplicit exact-count coverage remain
  passing. HostExplicit remains exactly 11 eligible names; `repo.commit` is
  ineligible.
- Public Tool names/schemas, PermissionLevel, selector authority, provider
  protocol, Trusted Profile schema, frontend, and Tauri permissions are
  unchanged. No persistence or member removal was added.
- Package/version/edition remain 13 packages, `0.25.0`, and Rust edition 2024.
  Cargo dependency manifests and `Cargo.lock` are unchanged. No live
  certification was run. The v0.25 immutable source remains
  `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.
- Exact-head CI for `fdc24c2` was run `34745160303` and passed. The final
  documentation closure head is `ed328a4`; its exact-head CI was run
  `34745248948` and passed all jobs.
