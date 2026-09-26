# Task 423C2 — Repository List Git Discovery Portability

Date: 2026-09-27
Status: **LOCAL VALIDATION PASS — COMMIT/PUSH/EXACT-HEAD CI PENDING**
Task type: minimal test-fixture portability correction; no publication

## Starting checkpoint and preserved CI failure

The verified starting checkpoint was:

```text
HEAD:        756377e4a2f664a67d4d8e48b3393400b00b199e
origin/master: 756377e4a2f664a67d4d8e48b3393400b00b199e
worktree:    clean
index:       clean
```

Task 423C1 stopped after exact-head Linux CI exposed a second deterministic
test portability defect. Preserve the historical run and do not rerun it:

```text
Run:      36276645054
Workflow: CI
Event:    push
SHA:      756377e4a2f664a67d4d8e48b3393400b00b199e
Runner:   Ubuntu 24.04
Format:   PASS
Check:    PASS
Boundary: corrected Unix repository-boundary test PASS
Tests:    FAIL — 334 passed, 7 failed, 0 ignored
Lint:     SKIPPED
Overall:  FAIL
```

The exact seven failing tests were:

- `repository_list::tests::deep_tracked_path_lists_in_ordinary_and_linked_worktrees`
- `repository_list::tests::definition_is_closed_and_execute_bound`
- `repository_list::tests::direct_child_saturation_is_deterministic`
- `repository_list::tests::nested_repository_fails_without_partial_entries_and_observation_does_not_mutate`
- `repository_list::tests::request_validation_and_target_failures_are_closed`
- `repository_list::tests::root_nested_direct_projection_and_visibility_are_bounded`
- `repository_list::tests::tracked_binary_file_is_listed_structurally_without_disclosing_contents`

All failed at `crates/rah-tools/src/repository_list.rs:312:72` with
`Os { code: 2, kind: NotFound, message: "No such file or directory" }`.
The test helper invoked `where.exe git.exe` unconditionally, but `where.exe`
is a Windows executable and is absent on Ubuntu. Fixture setup therefore
failed before exercising repository-list behavior. This is a test-fixture
Git discovery defect. It is not evidence of a production `repo.list`,
Git-layout identity, timeout, authority, or repository-observation semantic
defect.

## Existing repository precedent and correction

`crates/rah-tools/src/repository_search.rs` already implements the requested
cross-platform test `native_git()` behavior: Windows runs `where.exe git.exe`,
non-Windows runs `which git`, requires successful exit, takes the first
returned path, and canonicalizes it. Task 423C2 changes only the test helper
in `crates/rah-tools/src/repository_list.rs` to match that precedent. No
shared abstraction was introduced.

Source audit before validation confirmed that only the `repository_list`
test helper changed. No production Rust, test case, assertion, ignored or
cfg-disabled attribute, or `repository_search` source changed. All seven
previously failing tests remain enabled on Linux.

## Local validation

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS, exit 0. |
| `cargo check --workspace` | PASS, exit 0. |
| `cargo test -p rah-tools repository_list::tests:: -- --test-threads=1` | PASS, 8 passed, 0 failed, 0 ignored, 338 filtered. |
| `cargo test -p rah-tools --lib -- --test-threads=1` | PASS, 346 passed, 0 failed, 0 ignored. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS, exit 0. |
| `cargo metadata --no-deps --format-version 1` | PASS; 13 packages, 13 members, all `0.32.0`, edition `2024`. |
| `git diff --check` | PASS, exit 0. |

Invariant audit: no Cargo manifest or lockfile changed, so dependency/checksum,
feature, and workspace membership are unchanged. Workspace metadata remains
13 packages and 13 members, all version `0.32.0`, edition `2024`. The Rust
diff is restricted to the test helper; production `RepositoryListTool` and
`RepositoryObserver`, read-only `RepositoryObservation`, `HostExplicit`
count 11, authority, timeout policy, listing bounds, path validation, Git
command vectors, and Tauri permissions are unchanged. Windows local
validation cannot prove the non-Windows `which git` branch; exact-head Linux
CI is required.

## Commit, push, and exact-head CI

Pending. Record the intentional paths, exact commit SHA and subject, fetched
remote ancestry check, fast-forward push result, and new CI run ID/workflow/
event/tested SHA and job conclusions here. Do not rerun historical run
`36276645054`. If push or new exact-head CI fails, preserve it and stop.

## Publication and next task

No tag, GitHub Release, or publication is authorized in Task 423C2. Task 423D
and Task 424 have not started. Task 423D must reconcile the entire release
evidence chain and establish a fresh `READY_FOR_RELEASE` checkpoint before
Task 424 can begin.
