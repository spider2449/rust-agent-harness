# Task 381-B - `nearest_existing_ancestor()` Independent Security Re-audit

Date: 2026-09-22

## Verdict

**PASS - TASK 381-A BOUNDARY HARDENING VERIFIED**

This is an independent re-audit of the uncommitted Task 381-A correction. It
does not rewrite the Task 381 FAIL artifact, perform Task 382, or claim the
separate Task 382 Windows live recertification.

## Starting checkpoint and preserved state

- Starting `HEAD`: `ca96d1e5252bd4f320014981c562b6af27df71cf`.
- Task 379: `FAIL - LIVE CERTIFICATION BLOCKER`.
- Task 380: `PASS - PRODUCTION DEFECT CORRECTED`.
- Task 381: `FAIL - CORRECTION WEAKENS BOUNDARY / REMAINS INCORRECT`.
- Task 381-A: narrow uncommitted boundary hardening under audit.
- Existing Task 379, Task 380, Task 381, and Task 381-A artifacts were
  preserved.

The pre-existing worktree contained the Task 381-A production changes in
`crates/rah-tools/src/repository_boundary.rs`, `repository_list.rs`,
`repository_search.rs`, and the Desktop integration in `crates/rah-desktop`.
Task 381-B did not modify those files.

## Independent source review

The audited loop is `nearest_existing_ancestor()` in
`crates/rah-tools/src/repository_boundary.rs`:

```text
symlink_metadata(current)
  Ok(metadata) -> reject ambiguous; return ordinary directory; or error
  NotFound    -> ascend one lexical parent
  other error -> sanitized observation error
```

The exact `NotFound` branch is the only branch assigning
`current = current.parent()`. The successful-metadata branch first calls
`reject_ambiguous_component(&current)`, then returns `current` only when
`metadata.is_dir()` is true, and otherwise returns
`repository target ancestry is not a directory`. There is no `Ok(_) -> parent`
fall-through. The root-parent operation is not an authorization success path:
the root is returned by the helper when it is the first existing ordinary
directory, and the outer validator terminates at the selected root.

`validate_existing()` additionally checks containment before ancestry ascent,
rejects ambiguous and nested-marker components, and stops successfully only
at the selected repository root. Active root `.git` directory/gitfile targets
are rejected separately. Boundary errors are fixed sanitized messages and do
not include absolute paths or link targets.

## Existing-object and missing-depth matrices

The focused boundary tests passed on Windows, including the Windows junction
fixture. The source review and tests establish the following invariant for
one or many missing descendants: classification is determined by the first
existing object and never changes with missing depth.

| First existing object | One missing descendant | Many missing descendants |
| --- | --- | --- |
| ordinary directory | accepted as that directory | accepted as that directory |
| symlink | rejected | rejected |
| junction/reparse point | rejected | rejected |
| regular file | rejected | rejected |
| directory containing nested `.git` marker | rejected | rejected |

Safe ordinary-directory coverage includes `root/a` with omitted paths at
several depths and confirms the result is exactly `root/a`. The existing
`missing_descendant_uses_existing_ancestor_without_bypassing_boundaries` test
also covers a safe omitted path and a nested-marker omitted path.

The Unix symlink matrix covers `a/b/c/file.txt`, `a/x/missing.txt`,
`a/x/y/missing.txt`, and `a/x/y/z/missing.txt`, with an external sentinel;
all reject before the external target can be used. On Windows,
`existing_junction_ancestor_rejects_missing_descendant` created a real
junction to an external sentinel directory and rejected `a/b/c/file.txt`.
The Windows junction/reparse checks are also exercised by the broader
`rah-tools` and workspace suites.

The regular-file test uses `root/a` as a file and `root/a/b/c/file.txt` as the
candidate; it fails closed rather than climbing to `root`. Nested repository
tests reject both directory and file-form `.git` markers, including omitted
descendants.

## Root, Git metadata, and filesystem errors

- Root termination passed: `root/a/b/c/file.txt` resolves to `root` when
  `root` is the safe existing object; `root.parent()` is never accepted.
- Ordinary repository-root `.git` directory behavior passed.
- Linked-worktree repository-root `.git` gitfile behavior passed; active Git
  metadata is not treated as a nested repository boundary, but is not an
  authorized repository path target.
- Invalid/non-`NotFound` metadata input passed the fail-closed test. The
  implementation maps permission, sharing, unsupported, and other filesystem
  failures through the sanitized observation error rather than treating them
  as absence. No retry or replay guarantee is introduced.

## Sparse, deleted, and shared-tool regressions

The focused and full workspace suites passed the existing regression coverage
for:

- the original sparse-checkout reproducer: missing tracked candidate and
  missing immediate parent ascend to a safe existing ancestor, then classify
  as `changed_or_missing`/omitted rather than a boundary failure;
- multiple omitted sparse ancestors and deep tracked paths;
- deleted tracked paths with removed parent directories;
- `repo.list` direct-child projection, directory synthesis, saturation,
  ordering, file-as-directory rejection, tracked/ignored/untracked/deleted
  classification, invalid UTF-8, nested repositories, symlink/reparse
  rejection, submodule isolation, linked-worktree isolation, and projection
  conflicts; and
- `repo.search` path/text modes, `path_prefix`, literal and case-sensitive
  matching, current-worktree bytes, sparse/deleted omission, invalid UTF-8,
  binary/oversized handling, nested repositories, symlink/reparse rejection,
  linked-worktree isolation, and saturation.

The Windows live certification tests for Task 379 and Task 369 remained
explicitly ignored because their environment gates were not enabled. This
audit therefore verifies the correction and deterministic regression suites;
it does not substitute those ignored gates for Task 382 live certification.

## Containment and currentness

Containment regression passed for repository parent escape, outside paths,
sibling worktrees, external reparse targets, and common/private Git metadata.
The existing closed lexical path policy rejects `..`, `.`, absolute, UNC,
verbatim, drive-relative, ADS, and backslash-alias forms before or
independently of ancestor classification.

The linked-worktree matrix passed for main, linked A, and linked B. With a
selected worktree, missing/sparse candidates remain bounded to that selected
worktree; main, sibling B, common gitdir, and private gitdir are not accepted
as content ancestors.

Race review covers missing-to-existing symlink, ordinary-directory-to-reparse,
and existing-directory removal/replacement transitions. Later validation is
conservative and can reject after a transition. Semantics remain best-effort:
the implementation does not claim race freedom, rollback, or replay of
uncertain effects.

## Privacy and authority

Sanitized boundary and repository errors remained free of the nearest
existing absolute ancestor, repository root, external target, private gitdir,
common gitdir, and sibling worktree root. The privacy assertions passed in the
focused and workspace suites.

The authority review and tests confirm:

```text
repo.list  = ReadOnly / RepositoryObservation / Execute / repository_bound=true
repo.search = ReadOnly / RepositoryObservation / Execute / repository_bound=true
host_kind("repo.list") == None
host_kind("repo.search") == None
HostExplicit = exactly 11
```

No new ADR, permission level, authority category, dependency, or version
change was introduced. `cargo metadata` measured 13 packages, all at
`0.31.0`, with edition `2024`; no Cargo manifest or lockfile was changed.

## Validation evidence

All requested commands passed:

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

The first profile-composition attempt stopped at its documented fixture
precondition because `rah-mcp-echo-server.exe` was absent. The test-only
fixtures were then built using the repository-documented commands:

```text
cargo build -p rah-tools-mcp --bin rah-mcp-echo-server
cargo build -p rah-tools-plugin --bin rah-plugin-echo
```

The exact required profile-composition command was rerun and passed: 4 passed,
0 failed. No production source was changed to satisfy the precondition.

Measured focused results included:

- `rah-tools`: 339 passed, 0 failed; all integration and doc-test groups
  passed;
- `rah-profile-composition`: 4 passed, 0 failed;
- `rah-desktop`: 315 passed, 0 failed, 18 ignored;
- `rah-runtime-codex`: 85 passed, 0 failed, 1 ignored, plus 6 architecture
  and 11 live-gate contract tests passed; and
- the full workspace serial command passed all package, integration, and
  doc-test groups.

The historical test
`repository_diff_staged::unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty`
passed in both the focused `rah-tools` command and the full workspace command.
The historical timeout did not recur.

## Audit purity and Git state

Production Rust changed during Task 381-B: **no**. The four production files
shown by `git diff` were pre-existing Task 381-A/earlier-task work and were
preserved. The exact file added by Task 381-B is:

```text
docs/plans/2026-09-21-task-381-b-nearest-existing-ancestor-independent-security-reaudit.md
```

No commit, push, tag, version bump, reset, stash, or Task 382 work occurred.
`HEAD` remains `ca96d1e5252bd4f320014981c562b6af27df71cf`. The final status
retains the pre-existing Task 379/380/381/381-A artifacts and production
changes, plus this Task 381-B document only.

## Next authorized task

Task 382 - `repo.list` Windows Live Recertification.

It must start from a fresh disposable fixture and rerun the full certification
matrix; it must not merely resume at the old Task 379 failure point.
