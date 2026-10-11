# Task 251A — Host Prepare Reservation Finalization Fix

## Scope

Focused production correction for the HostExplicit async `repo.patch` Prepare
reservation state machine. No live rerun is authorized in Task 251A.

## Task 251 pre-effect live failure

The prior Task 251 live attempt stopped before ticket creation, HostExplicit
Started, Tool execution, and native replacement. It therefore established a
pre-effect failure only. The live gate remains ignored and pending a fresh
retry after Task 251A exact-head CI passes.

## Defect

Async `repo.patch` Prepare called `begin_prepare()`, entering
`HostPrepared(preparing)`, then later called the synchronous `prepare(ticket)`.
Because synchronous preparation requires `Idle`, the async path always returned
`Busy` and could not produce a prepared ticket.

## State-machine correction

Keep direct synchronous `prepare(ticket)` unchanged for the branch Prepare
path. Add `finalize_prepare(ticket)` with the exact transient-reservation
preconditions: `HostPrepared`, `preparing == true`, and no existing ticket. It
stores the ticket, clears `preparing`, and remains `HostPrepared`. Async
`repo.patch` Prepare now uses this finalize operation. Shared-preparation
failure still aborts the transient reservation back to `Idle`.

## Tests

Deterministic tests cover reservation exclusion, model and direct host
exclusion, transient `take_prepared`/cancel rejection, finalize success and
rejection cases, exact-once ticket consumption, abort behavior, unchanged
direct branch Prepare behavior, zero-effect shared patch preparation, failed
preparation cleanup, and unchanged HostExplicit provenance/conversation
behavior. The ignored Windows live gate is not executed.

## Validation

Validation completed successfully before the implementation commit:

- PASS `cargo fmt --check`
- PASS `cargo check --workspace`
- PASS `cargo test -p rah-desktop` (194 passed, 6 ignored)
- PASS `cargo test --workspace`
- PASS `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- PASS `node --check crates/rah-desktop/frontend/status.js`
- PASS `node crates/rah-desktop/frontend/status_authority_test.js`
- PASS `cargo build -p rah-desktop --release`
- PASS `git diff --check`
- PASS `cargo metadata --no-deps --format-version 1`

Metadata was confirmed as 13 packages, version `0.20.0`, edition 2024, with an
empty `Cargo.toml`/`Cargo.lock` diff. No live test was run.

## Commit

Commit: `fix: finalize reserved HostExplicit preparation`

## Exact-head CI

Before the implementation push, `origin/master` was
`4a47da65ffb86a2782a3b7fa46bd611e85a1b792`. CI run `34192605965` passed for
the implementation push at `089c9ed`. The final amended commit and its
exact-head CI result are recorded in the completion report.

## Follow-up

Task 251B — Resume Windows Live HostExplicit `repo.patch` Certification. Task
251B is not started automatically. Task 251 remains pending one fresh live
attempt after Task 251A CI PASS.
