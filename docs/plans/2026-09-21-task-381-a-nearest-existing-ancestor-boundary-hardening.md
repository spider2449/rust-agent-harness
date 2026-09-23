# Task 381-A - `nearest_existing_ancestor()` Boundary Hardening

Date: 2026-09-21

## Scope and verdict

This is a narrow production security hardening for the Task 381 blocker. It
does not redesign `repo.list`, broaden sparse/missing semantics, perform Task
382 Windows recertification, change authority, or create a commit.

Verdict: **PASS - NARROW BOUNDARY HARDENING COMPLETE**

This verdict means only that the correction is ready for the independent Task
381-B security re-audit. It does not restore Task 381 certification and does
not certify Windows live behavior.

## Starting state

- Starting `HEAD`: `ca96d1e5252bd4f320014981c562b6af27df71cf`.
- Task 379: `FAIL - LIVE CERTIFICATION BLOCKER`.
- Task 380: `PASS - PRODUCTION DEFECT CORRECTED`.
- Task 381: `FAIL - CORRECTION WEAKENS BOUNDARY / REMAINS INCORRECT`.
- Existing Task 379, Task 380, and Task 381 worktree artifacts were
  preserved.

## Task 381 failure and correction

Task 380 changed `validate_existing()` to call
`nearest_existing_ancestor()` when a tracked candidate is missing. The unsafe
branch was:

```rust
Ok(_) => current = current.parent()
```

When `root/a` was an existing symlink and `root/a/b/c/file.txt` was missing,
the helper reached `root/a`, treated its non-directory metadata as a reason to
return `root`, and allowed the outer validation loop to skip the ambiguous
component.

Task 381-A now classifies every successful metadata probe before any ascent:

- `reject_ambiguous_component()` rejects symlinks and Windows reparse points;
- an ordinary directory is returned as the nearest existing ancestor;
- an existing non-directory returns a sanitized boundary error; and
- only the accepted `NotFound` error ascends one lexical parent.

After `Ok(metadata)`, the helper returns success or error and never climbs
past that object. Other filesystem errors remain fail-closed.

## Regression evidence

The boundary tests cover:

- an ordinary `root/a` directory with multiple missing descendants returns
  exactly `root/a` and remains valid;
- an existing symlink ancestor rejects `root/a/b/c/file.txt` and additional
  missing descendant depths on Unix;
- an existing Windows junction/reparse ancestor rejects a missing descendant;
- an ordinary regular-file ancestor rejects without climbing to `root`;
- an existing nested repository marker remains rejected;
- a safe repository root is the last accepted ancestor and traversal does not
  escape to `root.parent()`; and
- a non-`NotFound` metadata error fails closed.

The shared `repo.list` and `repo.search` suites retain coverage for deep valid
paths, sparse and deleted tracked omissions, untracked/ignored exclusion,
direct-child projection, projection conflicts, nested repositories,
symlink/reparse rejection, path/text and `path_prefix` search behavior,
current-worktree bytes, and linked-worktree isolation. Their existing
authority/privacy tests remain in scope and were not changed.

The sparse/missing correction remains intact: a tracked candidate beneath
multiple absent parents is classified as `changed_or_missing` when its first
existing ancestor is an ordinary directory. Deleted tracked paths remain
omitted rather than producing a false boundary failure.

## Authority, privacy, and metadata

No ADR, dependency, version, authority, or tool contract changed. The
invariants remain:

```text
repo.list  = ReadOnly / RepositoryObservation / Execute / repository_bound=true
repo.search = ReadOnly / RepositoryObservation / Execute / repository_bound=true
host_kind("repo.list") == None
host_kind("repo.search") == None
HostExplicit = exactly 11
```

Production boundary errors remain sanitized and do not expose an absolute
ancestor, external link target, repository root, private gitdir, or common
gitdir. Best-effort currentness is preserved; no race-free or rollback claim
is made.

## Validation record

All requested validation commands completed successfully:

```text
cargo fmt --check                                      PASS
cargo check --workspace                                PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS
git diff --check                                       PASS
cargo metadata --no-deps --format-version 1           PASS
cargo test -p rah-tools -- --test-threads=1            PASS
cargo test -p rah-profile-composition                  PASS
cargo test -p rah-desktop -- --test-threads=1          PASS
cargo test -p rah-runtime-codex -- --test-threads=1    PASS
cargo test --workspace -- --test-threads=1             PASS
```

Measured focused totals were `rah-tools`: 339 passed, 0 failed;
`rah-profile-composition`: 4 passed, 0 failed; `rah-desktop`: 315 passed, 18
ignored, 0 failed; and `rah-runtime-codex`: 85 passed, 1 ignored, plus 6
architecture and 11 live-gate contract tests passed. The workspace rerun
also passed all these suites and all integration/doc-test groups. The
historical staged-diff test completed in 25.61 seconds in the focused
`rah-tools` run and no historical timeout recurred.

The focused and workspace `repo.list` regressions passed for deep valid paths,
sparse and deleted omissions, untracked/ignored exclusion, direct-child
projection, projection conflicts, nested repositories, symlink/reparse
rejection, and linked-worktree isolation. The focused and workspace
`repo.search` regressions passed for path/text modes, `path_prefix`, current
worktree bytes, sparse/deleted omission, symlink/reparse and nested-repository
rejection, and linked-worktree isolation.

`cargo metadata` measured 13 packages, all version `0.31.0`, edition `2024`.
No new dependency was introduced. The Windows live certification tests remain
ignored unless their explicit gates are enabled; this task does not claim
Windows live certification.

## Git and release boundaries

No commit, push, tag, version bump, release action, or Task 382 work was
performed. Task 381-B - `nearest_existing_ancestor` Independent Security
Re-audit remains required before Task 382 can proceed.
