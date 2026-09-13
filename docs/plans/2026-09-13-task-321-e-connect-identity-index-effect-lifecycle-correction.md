# Task 321-E — Connect Identity Currentness and Repository Index-Effect Lifecycle Correction

## Checkpoint and scope

- Task 321-D head: `2b83bc056af07ccf15a0c19cf615dac2a4409d21`
- Direct parent: `36d0390eb70f45e39014959abd3f81720296cc30`
- Task 321-D verdict: Verdict B — CORRECTION STILL REQUIRED
- Task 321-C writer coordination correction: closed at `f1602aceec9f0382fdb622c820b97864e781b92d`
- Task 320 activation invalidation atomicity: closed
- Accepted architecture: ADR 0027

This task is limited to F-321-D-1 Connect Commit-identity currentness and
F-321-D-2 asynchronous Stage/Unstage repository lifecycle reservation. It
does not add persistence, member removal, release work, live certification,
connected repository migration, HostExplicit eligibility, or ADR changes.

## Design

Connect will carry Commit identity generation through pending publication and
include it in the final lifecycle-gated currentness proof. The connected state
will retain the descriptive captured identity generation so a connected
composition built with an old identity is reported as requiring reconnect.
Identity change while connected continues to revoke pending Commit authority
and workflow; it does not perform live registry recomposition.

Stage and Unstage will use one Desktop-private, process-local reservation with
an opaque fresh token, repository generation, active member, exact repository
binding, and action kind. Reservation start is synchronous under
`lifecycle_coordination`: it rejects an existing reservation, validates the
current action/repository, revokes pending Commit authorization through the
nonblocking seam, consumes the observed action catalog, and publishes the
reservation. The lifecycle lock is released before target validation and Tool
execution. Activation, Disconnect, model configuration/reset, Commit identity,
and reviewed Commit authorization will reject while the reservation is active.

The reservation remains active through the single Stage/Unstage attempt and
the captured-repository workflow refresh. Completion rechecks the reservation
token, active member, repository generation, and exact repository Arc before
clearing it. Known failures and uncertain results are not retried or replayed.
Pre-reservation busy or invalid paths preserve the action/workflow; once a
reservation is established, existing one-attempt action consumption applies.

## Deterministic barrier matrix

Test-only barriers will cover:

| Boundary | Required proof |
| --- | --- |
| Connect preparation → identity update → final publication | stale Connect rejects; no old capability, registry, provider state, or Connected state is installed |
| Connect publication → identity update | writer serialization revokes old Commit authorization/workflow coherently |
| Stage/Unstage reservation → effect | activation and Disconnect are busy; A remains active; exactly one attempt proceeds |
| Activation → index reservation | stale A action is rejected with zero index attempts and B remains active |
| Index reservation → model/reset/identity/authorization | bounded busy and no generation, workflow, or authorization mutation |
| Effect terminal result → refresh/clear | token cleanup is safe on success, known failure, and uncertainty; refresh cannot write B workflow |
| Existing regressions | Task 321-C, 320, 317, 318, 315, and exact 11 HostExplicit names remain green |

## Validation and closure evidence

Run the requested focused Desktop tests and the complete workspace checks:
`cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`git diff --check`, metadata, Desktop release build, and the frontend/Tauri
tests. Record actual counts, changed files, dependency/lock status, exact-head
CI, and the explicit deferrals in the completion update.

## Implementation evidence

The correction is implemented in crates/rah-desktop/src/main.rs. Connect
captures identity generation in PendingConnectedPublication, compares the
complete five-field private tuple at both post-runtime and final publication
checks, and records the captured identity generation in connected state.
Identity changes therefore leave the old connected composition stale and
require reconnect; the existing writer revocation path still withdraws the
old Commit capability and workflow without live registry migration.

The Desktop index reservation is process-local and non-serializable. It keeps
the exact repository Arc, optional active member binding for compatibility
with legacy single-repository fixtures, repository generation, action kind, and
a checked monotonic token. Activation and the conflicting lifecycle writers
return bounded busy while it exists. The reservation remains through target
validation, one Tool attempt, and captured-A workflow refresh; the token and
binding are checked before cleanup. No blocking lifecycle guard crosses an
await, and uncertain Tool outcomes are not retried or replayed.

Deterministic coverage added in this task: one Connect identity-barrier
currentness test, Stage and Unstage reservation-versus-activation tests, an
old-completion/new-token cleanup test, and existing Desktop regressions retained
green. Focused result: 4 passed, 0 failed, 244 filtered. Full Desktop result:
238 passed, 10 ignored. Full workspace test command passed; its notable
aggregates were rah-model 5, rah-profile-composition 4, rah-protocol 13,
rah-runtime 7, rah-runtime-codex 83 passed/1 ignored, rah-sandbox 11 passed/1
ignored, rah-session 6, rah-tools 288 passed, rah-tools-mcp 1,
rah-tools-plugin 21, and the Desktop aggregate above. The workspace contains
13 packages at version 0.25.0, edition 2024; no Cargo manifest or lockfile
changed.

Additional validation passed: cargo fmt --check, cargo check --workspace,
cargo clippy --workspace --all-targets --all-features -- -D warnings,
git diff --check, cargo metadata --no-deps --format-version 1,
cargo build -p rah-desktop --release,
node crates/rah-desktop/frontend/status_authority_test.js,
node crates/rah-desktop/frontend/repository_membership_test.js, and
node crates/rah-desktop/tauri_permission_test.js.

The exact v0.25.0 immutable source remains
a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4. No persistence, member removal,
release work, live certification, frontend authority, Tauri permission, public
Tool/schema, HostExplicit, Cargo, or ADR change is part of this task.
