# Task 380 - `repo.list` Windows boundary failure diagnosis and narrow correction

Date: 2026-09-20

## Verdict

**PASS - PRODUCTION DEFECT CORRECTED**

This is a focused diagnosis and correction result, not a renewed Task 379
Windows live-certification result. The Task 379 record remains explicitly
`FAIL - LIVE CERTIFICATION BLOCKER`.

## Starting checkpoint and preserved failure

The task started at:

```text
ca96d1e5252bd4f320014981c562b6af27df71cf
feat: add bounded repository structure listing
```

Task 379's preserved live failure was:

```text
{}
                PASS
{"path":"crates"}        PASS
{"path":"crates/alpha"} PASS
{"path":"crates/alpha/src"} FAIL

Git repository policy rejected capability:
repository boundary observation failed
```

The original Task 379 evidence was not rewritten. The current worktree began
with only its Desktop harness/evidence changes; no commit, push, tag, or
version bump was performed.

## Smallest reproducer and comparison

The fresh native Git reproducer used only:

```text
repo/
  a/
    b/
      c/
        file.txt
```

`a/b/c/file.txt` was tracked and the same four requests were executed in an
ordinary main worktree and a linked-A worktree:

```text
{}
{"path":"a"}
{"path":"a/b"}
{"path":"a/b/c"}
```

All requests passed in both worktrees. The ordinary and linked-A roots both
had ordinary directory components and an ordinary regular file. Windows
metadata showed `Directory` for the root and `a`, `a/b`, and `a/b/c`, and
`Archive` for `file.txt`; `LinkType` and `Target` were empty. `fsutil
reparsepoint query` returned error 4390 (not a reparse point) for every
component.

Linked-worktree topology was separately confirmed in form only: ordinary main
used a `.git` directory, linked A used a root `.git` file referring to a
private gitdir, and the private gitdir used its commondir record. This was not
the rejecting condition.

The minimal clean fixture therefore did not reproduce the original failure.
Progressive comparison of the Task 379 fixture showed the additional
condition: after the sparse-checkout gate removed the tracked `saturation`
directory from the current worktree, Git's tracked inventory still contained
its tracked files. The subsequent root listing reproduced the failure while
validating a missing tracked file beneath the now-missing `saturation` parent.

## Layer diagnosis

The lowest failing behavior was the shared repository-boundary target
validation used by the direct observer tools:

1. Direct `RepositoryNestedBoundaryPolicy` validation failed for a tracked
   path whose intermediate parent had been removed by sparse checkout.
2. Direct `RepositoryListTool` and `RepositorySearchTool` reproduced the
   same missing-ancestor behavior; the clean deep-path matrix passed.
3. Effective/profile composition constructed the same closed repository-bound
   `repo.list` definition, and the ToolRegistry dispatch remained successful
   for the corrected focused path.
4. The Windows Desktop active-repository production registry path passed the
   corrected normal navigation and sparse-omission gate.
5. The Generic Tool Bridge regression remained covered by its existing
   production-dispatch test and was rerun after correction.

The failure was not caused by linked-worktree `.git`/commondir metadata, a
requested-subtree policy change, directory projection conflict hardening, or
fixture contamination by the later nested/reparse fixtures. The later hostile
fixtures are created after the sparse gate; the failing call was the sparse
omission rerun.

## Exact rejecting path and root cause

The exact path was:

```text
RepositoryListTool::execute_bounded
 -> RepositoryObserver::validate_target
 -> RepositoryNestedBoundaryPolicy::validate_existing
 -> reject_ambiguous_component
 -> fs::symlink_metadata
```

Before correction, `validate_existing()` handled a `NotFound` target by using
only `path.parent()` as `current`. When the tracked target and that parent
were both absent after sparse checkout, `reject_ambiguous_component()` called
`fs::symlink_metadata()` on the absent `saturation` directory. Windows returned
`ERROR_FILE_NOT_FOUND` (code 2), which was sanitized by
`boundary_observation_error()` to `repository boundary observation failed`.

Root-cause classification: **D - shared candidate-helper regression**.

The shared target validator treated a missing tracked candidate's immediate
parent as though it had to exist. This made valid sparse/missing tracked
inventory candidates fail before `repo.list` or `repo.search` could classify
them as `changed_or_missing`.

## Narrow correction

`validate_existing()` now finds the nearest existing ancestor when the target
itself is missing. It still applies the existing ancestry checks to that
ancestor and every ancestor up to the selected repository root. Existing
nested `.git` markers, symlinks, junctions/reparse points, outside ancestry,
and metadata read errors remain fail-closed. No boundary policy was skipped,
and no lexical-only or follow-link behavior was introduced.

## Regression coverage

Added deterministic coverage for:

- ordinary-main and linked-A deep `repo.list` navigation through
  `a/b/c/file.txt`;
- a missing tracked descendant whose parent is also absent, reported as
  `changed_or_missing` rather than a boundary failure;
- direct boundary validation of a missing descendant while retaining nested
  `.git` rejection beneath the existing ancestor;
- `repo.search` path omission for the same missing tracked parent case.

Existing adversarial coverage remained green for nested repositories, Windows
reparse/junction boundaries, symbolic links, submodule/gitlink handling,
invalid UTF-8, tracked-only visibility, linked-worktree isolation, and
conflicting file/directory projection. Existing `repo.search` path/text,
literal/case-sensitive, prefix, current-worktree, omission, binary/size,
linked-isolation, nested-boundary, and saturation behavior remained green.

## Focused Windows evidence after correction

The real Task 379 active linked-A Desktop registry was rerun only through the
affected gate. It passed:

```text
{}
{"path":"crates"}
{"path":"crates/alpha"}
{"path":"crates/alpha/src"}
```

The sparse-checkout omission gate that previously triggered the boundary
failure also passed. This focused run stopped before the remaining Task 379
security and certification matrix; it did not re-earn full Task 379
certification.

## Authority and metadata

No authority definitions were changed. `repo.list` remains:

```text
ReadOnly
RepositoryObservation
Execute
repository_bound=true
```

`HostExplicit` remains exactly 11, with `host_kind("repo.list") == None` and
`host_kind("repo.search") == None` in the applicable deterministic authority
coverage. No ADR, permission level, provider boundary, or dependency edge was
added or changed.

Workspace metadata remained 13 packages, all `0.31.0`, edition 2024.

## Validation

The requested validation passed:

```text
cargo fmt --check                                       PASS
cargo check --workspace                                 PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
                                                         PASS
git diff --check                                        PASS
cargo metadata --no-deps --format-version 1            PASS
```

Metadata was `packages=13`, `versions=0.31.0`, `editions=2024`.

Focused deterministic gates passed:

- `cargo test -p rah-tools -- --test-threads=1`: 335 unit tests passed and
  all repository integration tests passed.
- `cargo test -p rah-profile-composition`: 4 tests passed.
- `cargo test -p rah-desktop -- --test-threads=1`: 315 passed, 18 ignored.
- `cargo test -p rah-runtime-codex -- --test-threads=1`: 85 passed, 1
  ignored; architecture and live-contract integration tests passed.
- `cargo test -p rah-runtime-codex repository_list_dispatches_through_the_generic_bridge
  -- --test-threads=1`: passed.
- `cargo test --workspace -- --test-threads=1`: passed with exit code 0. All
  workspace unit, integration, and doc-test groups completed without failure
  or timeout. Explicit live-certification tests remained ignored unless their
  separate environment gates were set.

No live model, Codex baseline, release, or publication claim is made here.

## Changed files

- `crates/rah-tools/src/repository_boundary.rs` - nearest-existing-ancestor
  correction and direct boundary regression.
- `crates/rah-tools/src/repository_list.rs` - ordinary/linked deep-path and
  sparse omission regressions.
- `crates/rah-tools/src/repository_search.rs` - shared missing-parent omission
  regression.
- `crates/rah-desktop/src/main.rs` - preserved Task 379 live harness/evidence
  only; no Task 380 production Desktop logic change.
- `docs/plans/2026-09-20-task-379-repository-structure-listing-windows-live-certification.md`
  - preserved historical Task 379 failure record.
- this document.

## Disposition

Full Task 379 Windows certification has not yet been re-earned. The next
recommended task is:

```text
Task 381 - repo.list Narrow Correction Independent Re-audit
```

Only after Task 381 passes should Task 382 restart the complete fresh-fixture
Windows live-certification matrix.

## Git state

`HEAD` remains `ca96d1e5252bd4f320014981c562b6af27df71cf`. The worktree retains
the Task 379 certification harness/evidence and the Task 380 production
correction, regressions, and diagnosis record. No commit, push, tag,
version-bump, reset, cleanup, or release action occurred.
