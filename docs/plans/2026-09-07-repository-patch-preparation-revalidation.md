# Task 248A — Repository Patch Preparation Revalidation

## Status

Implementation complete.

## Boundary

This follow-up adds capability-specific, zero-effect revalidation to the
existing `rah-tools` `RepositoryPatchPreparer`. Desktop, runtime, protocol,
Cargo, ADR, frontend, and workflow files remain out of scope.

## Contract

`RepositoryPatchPreparer::revalidate` accepts an opaque
`RepositoryPatchPreparation` and uses the existing repository lease. It
returns success only when the preparation's preparer, repository, Git, target,
parent, preimage, Git state, canonical input, postimage, review, and review
identity still match current observations. Every failure returns the
sanitized `RepositoryPatchPreparationError::Stale` result.

Revalidation does not execute a Tool, create a temporary patch file, call the
replacement primitive, write the worktree or index, or change HEAD, refs,
Stage, or Commit state.

## Deterministic coverage

Tests cover unchanged success; changed bytes; same-byte target replacement;
dirty and staged targets; HEAD/ref changes; cross-preparer rejection;
test-private postimage/review mismatch; zero replacement attempts on every
revalidation path; and absence of `.rah-repo-patch-*.tmp` artifacts.

## Validation

Required validation was run sequentially:

```text
cargo fmt --check
cargo check --workspace
cargo test -p rah-tools repository_worktree_patch -- --nocapture
cargo test -p rah-tools
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
git diff -- Cargo.toml Cargo.lock
```

Metadata must report 13 packages, version `0.20.0`, edition 2024, and the
Cargo diff must be empty.

Results: `cargo fmt --check`, `cargo check --workspace`, the focused
`repository_worktree_patch` suite (37 passed), `cargo test -p rah-tools`
(208 unit tests passed and integration suites passed), `cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
`git diff --check` passed. Metadata reported 13 packages, all `0.20.0`,
edition 2024; the Cargo diff was empty.

## Checkpoint

Starting `HEAD == origin/master` is
`6436b5258e00f72f92bbd0dc3aa34e753c17090e`. Required current-head CI is
`34101660727`, completed successfully for that exact commit. Final publication
and exact-head CI for this task will be recorded after the implementation
commit.

## Commit

Preferred message: `feat: add repository patch preparation revalidation`.

## Next task

Task 249 — Desktop HostExplicit `repo.patch` Backend Integration. It is not
started automatically.
