# Task 376 - Bounded Repository Structure Listing Production Implementation

Date: 2026-09-20
Status: IMPLEMENTATION COMPLETE - INTENTIONALLY UNCOMMITTED FOR TASK 377
Task type: production implementation

## Starting state

```text
HEAD = 093d156d0f4cb3bc57ba0d13dcc0bc4b2d9ae1b1
parent = 47f5de5
origin/master = 2f9bd83957a7cefbd83e2ca377fd2125b8376616
worktree = clean
```

The implementation is intentionally uncommitted and will not be pushed. Task
377 independent security/currentness audit has not been performed.

## Changed files

```text
crates/rah-desktop/src/effective_authority.rs
crates/rah-desktop/src/host_invocation.rs
crates/rah-desktop/src/main.rs
crates/rah-profile-composition/src/lib.rs
crates/rah-runtime-codex/src/bridge_tests.rs
crates/rah-tools/src/lib.rs
crates/rah-tools/src/repository_list.rs
crates/rah-tools/src/repository_observer.rs
crates/rah-tools/src/repository_search.rs
crates/rah-tools/src/trusted_profile.rs
docs/plans/2026-09-20-task-376-bounded-repository-structure-listing-implementation.md
```

## Frozen contract implemented

`repo.list` is a separate closed Tool. `{}` lists the selected active
repository root; `{"path":"tracked/directory"}` lists direct children only.
The Tool uses tracked Git inventory, current selected-worktree eligibility,
repository-relative UTF-8 paths, synthesized directory entries, `file` and
`directory` kinds, deterministic byte ordering, `best_effort` consistency,
128-entry saturation, and the frozen omission/error vocabulary and bounds.
Its authority is `ReadOnly / RepositoryObservation / Execute` with
`repository_bound=true`. HostExplicit remains exactly 11 and excludes
`repo.list`.

## Shared helper and production surfaces

The private observer module now owns the shared fixed tracked-inventory result
validation, NUL parsing, bounded logical path normalization, repository target
mapping, and no-follow current candidate classification used by both
`repo.search` and `repo.list`. No generic public inventory API or second Git
inventory command was added.

Production registration is explicit in the trusted profile observer family,
effective profile composition, the active selected-repository Desktop registry,
and Desktop Effective Authority. The Generic Tool Bridge uses the existing
registry route. No HostExplicit kind, permission level, authority category,
dependency, version, ADR, or release artifact was added.

## Tests and validation

Added deterministic listing tests for closed schema and validation, root and
nested direct projection, current tracked visibility, untracked/ignored
absence, deterministic saturation, nested boundary failure, and no intentional
mutation. Added profile/composition, Desktop registry/authority, and Generic
Tool Bridge coverage. Existing `repo.search` tests run against the shared
helper and remain green; no search schema, bounds, output, or semantics were
intentionally changed.

Focused validation passed:

```text
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo metadata --no-deps --format-version 1
cargo test -p rah-tools -- --test-threads=1       (331 passed, 0 failed)
cargo test -p rah-profile-composition -- --test-threads=1 (4 passed)
cargo test -p rah-desktop -- --test-threads=1     (315 passed, 17 ignored)
cargo test -p rah-runtime-codex -- --test-threads=1 (85 passed, 1 ignored)
git diff --check
```

The required serial workspace command was executed. It passed through the
Desktop, runtime, and most `rah-tools` tests, then one existing staged-diff
fixture timed out under the long aggregate run:

```text
cargo test --workspace -- --test-threads=1
failure: rah-tools repository_diff_staged::unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty
error: repository observation exceeded its total timeout
```

The exact failing test was rerun in isolation and passed. This transient
aggregate-run failure is reported rather than treated as a passing workspace
gate.

Metadata verification reported 13 workspace packages, all version `0.31.0`,
edition `2024`, with no dependency or lockfile drift.

## Deviations and nonclaims

No known implementation deviation from Task 375. The result remains
best-effort and does not claim a transaction, snapshot semantics, zero
incidental filesystem writes, network isolation, rollback, or live model
selection/certification. The workspace validation caveat above is a validation
result, not a contract deviation.

## Closure boundary

Task 377 - Repository Structure Listing Independent Security / Currentness
Audit is the recommended next task and has not been started automatically.
