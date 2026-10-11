# Task 381 — `repo.list` Narrow Correction Independent Re-audit

## Verdict

**FAIL — CORRECTION WEAKENS BOUNDARY / REMAINS INCORRECT**

The Task 380 correction fixes the sparse/missing-parent false rejection for
ordinary directories, but it does not preserve the existing-link boundary. An
existing symlink ancestor can be skipped while walking upward to its parent.
That violates the required `root/a -> external` plus missing descendant case.
Task 382 must not begin until this narrow helper defect is corrected and
independently re-audited.

## Starting state

- Starting checkpoint and current `HEAD`: `ca96d1e5252bd4f320014981c562b6af27df71cf`.
- Task 379: **FAIL — LIVE CERTIFICATION BLOCKER**.
- Task 380: **PASS — PRODUCTION DEFECT CORRECTED**.
- Existing worktree changes were preserved. No commit, push, tag, version
  bump, reset, cleanup, or Task 382 work was performed.

## Correction audited

The only production correction in `crates/rah-tools/src/repository_boundary.rs`
changes the `validate_existing()` `NotFound` arm. The former immediate-parent
step now calls `nearest_existing_ancestor(path)`.

The helper probes the candidate with `fs::symlink_metadata`; it:

1. returns an existing directory;
2. returns the parent of any existing non-directory;
3. ascends only for `ErrorKind::NotFound`; and
4. maps every other filesystem error to the sanitized boundary observation
   failure.

The returned path then goes through the unchanged `validate_existing()` loop:
`paths_equivalent` root termination, lexical `is_beneath` containment,
`reject_ambiguous_component`, and `reject_nested_marker`.

The root `.git` exception is unchanged: the loop returns before checking the
selected root itself, while `is_active_git_metadata` still rejects the root
metadata path as a target. Root `.git` directory and linked-worktree `.git`
file tests remained green.

## Safe missing candidates

The deterministic boundary test
`missing_descendant_uses_existing_ancestor_without_bypassing_boundaries`
passed. A candidate with multiple absent components, equivalent to
`root/sparse/omitted/file.txt`, reaches the existing repository root and is
accepted for caller classification. A missing descendant beneath an existing
nested `.git` marker remains rejected.

The list regression `deep_tracked_path_lists_in_ordinary_and_linked_worktrees`
passed for ordinary and linked worktrees. Removing the tracked `a` subtree
caused the root projection to omit it with `changed_or_missing`, rather than a
boundary failure. Search regression
`sparse_omitted_tracked_parent_is_reported_not_rejected` also passed and
returned no match with `omitted.changed_or_missing == 1`.

These results establish the intended ordinary-directory sparse behavior, but
the Task 379 Windows live certification test was intentionally not rerun:
this task is an independent audit, not full Windows certification.

## Root termination and lexical containment

For a validated repository-relative target, the helper monotonically removes
one path component per `NotFound` step. The outer loop accepts only the
selected root or a path lexically beneath it; it never accepts `root.parent()`.
If the selected root disappears during the walk, the first existing ancestor
outside it is rejected by `is_beneath`.

The list/search request validators reject empty, `.`, `..`, absolute, leading
backslash, backslash-containing, colon/drive-relative/ADS-like, NUL, and
case-equivalent `.git` components before `repository_target_path` probes the
filesystem. The existing mutation target validators similarly require normal
components. No new filesystem normalization or path authority was introduced
by Task 380.

This protects the production callers, but `validate_existing()` itself is an
internal host-path primitive rather than a standalone lexical request parser.
The correction does not broaden that contract.

## Boundary adversarial results

### Existing symlink ancestor — blocker

The required structure is:

```text
root/a -> external
root/a/b/c/file.txt  (missing)
```

The line-by-line execution is unsafe:

1. the candidate and `b/c` return `NotFound`;
2. `symlink_metadata(root/a)` succeeds, but reports a symlink and therefore
   does not satisfy `metadata.is_dir()`;
3. the `Ok(_)` arm returns `root/a.parent()`, namely `root`;
4. `reject_ambiguous_component(root/a)` is never called; and
5. the outer loop accepts `root`.

Thus the missing descendant can be treated as a safe candidate without
rejecting the existing symlink ancestor. This is a direct violation of the
required symlink result and is sufficient for the FAIL verdict. The temporary
Windows probe was removed after execution; directory symlink creation was
unavailable on this host due to insufficient privilege, so there is no claim
of live Windows symlink creation. The source-level counterexample does not
depend on that privilege.

### Junction/reparse ancestors

Existing deterministic Windows junction/reparse tests passed, including
linked-worktree layout, multi-file preflight, and repository-boundary
coverage. An existing directory junction normally satisfies the directory
branch and is then rejected by `reject_ambiguous_component` before the walk
can skip it. This is positive evidence for directory reparse points, but it
does not repair the symlink/non-directory skip branch and does not justify a
PASS.

### Reparse below a safe ancestor

The unchanged validation loop checks the nearest returned existing directory
before ascending. Therefore an existing `root/a/b` reparse directory is
rejected at `b` rather than skipped to `a`; the deterministic junction and
reparse matrix remained green. The symlink counterexample shows that this
property is not true for every ambiguous non-directory ancestor.

### Nested repositories

Existing nested repository tests passed for ordinary nested `.git` directories,
`.git` files, and Windows case-equivalent `.GIT` markers where the platform
test was applicable. If the nearest existing ancestor is `a/b` and it has a
nested marker, `reject_nested_marker(a/b)` still rejects before ascending.
The missing target does not suppress that check. The independent boundary
integration test for repository A plus nested repository B also passed.

### Linked worktrees

Linked-worktree isolation tests passed for list, search, observers, Git layout,
mutation, and Desktop composition. Selected A remained isolated from main and
B; shared common Git storage did not become candidate authority. The existing
Windows live list test remains ignored and was not rerun under this task’s
non-certification scope.

### Deleted and sparse paths

Deleted tracked descendants were omitted as `changed_or_missing` in the
deterministic list/search coverage; no false boundary error was observed.
Sparse/missing tracked candidates were similarly omitted. Directory synthesis
did not disclose deleted or sparse descendants.

## Filesystem errors and currentness

`nearest_existing_ancestor` ascends only on `NotFound`. PermissionDenied,
sharing violations, invalid-name errors, and other I/O failures are converted
to the sanitized `repository boundary observation failed` error. No arbitrary
error is converted into a missing candidate.

The race analysis is conservative for ordinary directories: an ancestor that
disappears before the next metadata/read-directory check fails closed; a
directory replaced by a reparse point is rejected; a candidate appearing
after inventory is revalidated and classified on the observed state; and an
unsafe link appearing as a missing parent is rejected if it is observed as a
directory/reparse component. There is no retry, replay, rollback, or snapshot
claim.

The symlink skip is a currentness/boundary defect because the revalidation
sequence can observe the link but discard it before the ambiguity check.

## `repo.list` regression matrix

The serial `rah-tools` and Desktop suites kept the existing list coverage
green: root and deep paths, direct-child projection, synthesized directories,
file-target rejection, nonexistent logical targets, current/modified/staged
state handling, untracked/ignored exclusion, deleted and sparse omission,
deterministic ordering, saturation, projection conflict fail-closed behavior,
nested boundary, linked-worktree isolation, privacy, and authority mapping.

The result is **green regression behavior with one uncovered/failed adversarial
boundary case**, not a security PASS.

## `repo.search` regression matrix

The serial `rah-tools`, Desktop, and runtime-codex suites kept path/text mode,
literal case-sensitive matching, `path_prefix`, current-worktree content,
deleted and sparse omission, binary/oversized/invalid-UTF-8 omission,
saturation, nested-boundary rejection, linked-worktree isolation, generic
bridge dispatch, and privacy tests green.

The shared helper remains unsafe for the existing symlink-ancestor case, so
search cannot be treated as security-cleared independently of the list FAIL.

## Privacy and authority

Sanitized production errors remained bounded; no nearest absolute ancestor,
repository root, external target, private Git directory, or common Git
directory appeared in ToolOutput in the passing list/search privacy tests.

Authority remained unchanged:

- `repo.list`: `EffectClass = ReadOnly`;
- `AuthorityCategory = RepositoryObservation`;
- `PermissionLevel = Execute`;
- `repository_bound = true`;
- `host_kind("repo.list") == None`;
- `host_kind("repo.search") == None`;
- HostExplicit remained exactly the existing 11 names.

No ADR, dependency edge, permission level, authority category, profile schema,
or HostExplicit entry changed.

## Validation

All required commands completed successfully before the adversarial source
trace was recorded:

```text
cargo fmt --check                                      PASS
cargo check --workspace                                PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS
git diff --check                                       PASS
cargo metadata --no-deps --format-version 1           PASS
cargo test -p rah-tools -- --test-threads=1           PASS
cargo test -p rah-profile-composition                 PASS
cargo test -p rah-desktop -- --test-threads=1         PASS
cargo test -p rah-runtime-codex -- --test-threads=1   PASS
cargo test --workspace -- --test-threads=1             PASS
```

Focused totals:

- `rah-tools`: 335 unit tests passed; all integration and doc tests passed.
- `rah-profile-composition`: 4 passed.
- `rah-desktop`: 315 passed, 18 ignored, 0 failed.
- `rah-runtime-codex`: 85 passed, 1 ignored, 0 failed, plus architecture and
  live-gate contract tests passed.

The full serial workspace command exited 0. Its repeated historical staged-diff
test passed again (`repository_diff_staged::unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty`,
25.57 seconds in the final workspace run). The prior timeout did not recur.

The temporary symlink-ancestor probe was not part of the final worktree. Its
Windows directory-symlink creation reported insufficient privilege and was
removed; that absence is explicitly not counted as Windows live symlink
certification.

## Metadata and Git state

`cargo metadata` reported 13 packages, all version `0.31.0`, edition 2024, with
no dependency drift. `HEAD` remains
`ca96d1e5252bd4f320014981c562b6af27df71cf`.

The final working tree contains the pre-existing Task 379/380 evidence and
correction plus this Task 381 audit artifact. No commit, push, tag,
version-bump, or release action occurred.

## Recommended next task

Do not begin Task 382 yet. First perform a narrow Task 381 follow-up that fixes
`nearest_existing_ancestor` so every existing ancestor, including symlink and
other ambiguous non-directory/reparse forms, is validated before it is skipped;
then rerun this independent adversarial audit. Only after that audit reaches a
passing verdict should the next authorized task be:

```text
Task 382 — repo.list Windows Live Recertification
```
