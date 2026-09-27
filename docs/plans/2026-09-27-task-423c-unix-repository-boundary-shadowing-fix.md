# Task 423C — Unix Repository Boundary Test Shadowing Fix

Date: 2026-09-27
Status: **Task 423C1 CI failure retained; Task 423C2 follow-up PASS**
Task type: minimal test-source compiler correction; no publication

## Starting state and preserved evidence

Starting HEAD and `origin/master`:

```text
0301a52095b1b7c3717a6497f4f2dd042b184bae
```

Task 423B remains **STOP — EXACT-HEAD CI FAILED; NOT READY FOR PUBLICATION**.
At task start the only dirty paths were the Task 423B post-CI updates to
`docs/RAH_V0.32_RELEASE_GATE.md` and
`docs/plans/2026-09-26-task-423b-v0.32-release-preparation-finalization.md`;
the index was clean and there were no dirty source paths. Before Task 423C
edits, their tracked diff fingerprint was
`4b8a243ee6b53e7655f45d11efe81ce8b9a0be93` from
`git diff --binary | git hash-object --stdin`. The individual working-file
SHA-256 fingerprints were:

```text
docs/RAH_V0.32_RELEASE_GATE.md
057561ef307918c2201686bb398d9ea6d1dae0be1325aa50ff16c104f0e856bc
docs/plans/2026-09-26-task-423b-v0.32-release-preparation-finalization.md
567da48e0814167d4c3ee2458486eaf53a3ba26a8f1243f3843fc14764eb170e
```

The starting state and original failure are retained as evidence; Task 423B
is not rewritten as PASS.

## Frozen Task 423B CI failure

```text
Run ID: 36275076398
Workflow: CI
Event: push
Tested SHA: 0301a52095b1b7c3717a6497f4f2dd042b184bae
Job: deterministic-validation
Checkout: PASS
Rust toolchain: PASS
Formatting: PASS
Workspace check: PASS
Workspace test: FAIL
Workspace lint: SKIPPED
```

GitHub Actions failed while compiling the `rah-tools` tests on Ubuntu with:

```text
error[E0618]: expected function, found std::path::PathBuf
crates/rah-tools/src/repository_boundary.rs:332:24
```

The test is guarded by `#[cfg(unix)]`. Its local `let root = root(...)`
binding shadows the `root(name: &str) -> PathBuf` helper before the second
`root("symlink-ancestor-target")` call. Windows release-preparation checks do
not compile this Unix-only test, so a Windows PASS cannot establish that this
branch compiles or runs. This is compile-time test-name shadowing, not a
runtime Git-layout failure.

## Correction and source audit

The local repository-root binding in
`existing_symlink_ancestor_rejects_all_missing_descendant_depths` is renamed
from `root` to `root_path`; only references belonging to that binding are
updated. The helper `fn root(name: &str) -> PathBuf` remains unchanged. The
neighboring Windows junction test remains unchanged. Descendant cases,
assertions, symlink operations, and cleanup semantics remain unchanged. No
production Rust source, authority, permissions, Git-layout validation,
timeouts, `repo.list` behavior, dependencies, Cargo metadata, workspace
version, or release version are changed.

Rust source diff audit: the only Rust path changed is
`crates/rah-tools/src/repository_boundary.rs`. The failing test remains
`#[cfg(unix)]`; the helper `fn root(name: &str) -> PathBuf` was not changed;
the neighboring Windows junction test was not changed; only the local binding
and its references changed. All four descendant cases, assertions, symlink
operations, and cleanup calls remain present. No test coverage was removed or
weakened. The source diff contains no production implementation change.

## Validation

Windows local validation is collateral-change evidence only; it does not
compile or execute the `#[cfg(unix)]` test. The corrected Unix compile and
execution must be established by new exact-head Linux CI.

| Command | Result |
| --- | --- |
| `cargo fmt --check` | **FAIL**; exit 1. Rustfmt requests multiline formatting of the updated `assert!` at `repository_boundary.rs:340`; complete output was: `Diff in ...repository_boundary.rs:340: ... - assert!(policy .validate_existing(&root_path.join(descendant)) .is_err()); + assert!( policy .validate_existing(&root_path.join(descendant)) .is_err() );`. Per task stop rule, remaining local commands were not run. |
| `cargo check --workspace` | Pending |
| `cargo test -p rah-tools --lib -- --test-threads=1` | Pending |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pending |
| `cargo metadata --no-deps --format-version 1` | Pending |
| `git diff --check` | Pending |
| Final package/member/version/edition/dependency/feature/authority audit | Pending |

STOP rule applied after the first local validation failure. No later local
check was run, no large Task 423/423A matrix was repeated, and no commit,
push, or new CI run was started.

## Commit, push, and exact-head CI

Correction commit: **NOT CREATED**.
Push result: **NOT ATTEMPTED**.
Post-push `HEAD == origin/master`: not applicable; starting relationship
remains the last verified one.
New exact-head Linux CI: **NOT STARTED**.

The failed Task 423B run will not be rerun. CI failure, if any, is preserved
and stops this task; there is no retry-until-green.

## Release and next task

No tag, GitHub Release, or publication has occurred. Task 424 has not started.
Task 423C stopped before commit or CI. After the local formatting failure is
resolved and all required checks pass, a later authorized closeout must
establish corrected exact-head CI and release readiness in a separate step:

```text
Task 423D — Finalize v0.32.0 Release Readiness After CI Correction
```

Task 423D must reconcile the final release records, verify the corrected
exact source checkpoint, and establish `READY_FOR_RELEASE` before Task 424.

## Task 423C1 continuation — resumed local validation

Task 423C1 continues the same Task 423C stabilization chain after its
formatting-only stop. The original `cargo fmt --check` failure above remains
part of the record: it reported multiline formatting for the updated
assertion, and Task 423C stopped before running any later required local
check. Task 423C1 applied Rustfmt and then resumed the checks in order.

```text
cargo fmt: PASS (exit 0)
```

The post-format source diff contains only the already-recorded local binding
rename and its references, plus Rustfmt's multiline layout of that test's
assertion. No other source path changed.

| Command | Result |
| --- | --- |
| `cargo fmt --check` | **PASS**, exit 0 after `cargo fmt`. |
| `cargo check --workspace` | **PASS**, exit 0; finished in 33.14s. |
| `cargo test -p rah-tools --lib -- --test-threads=1` | **PASS**, exit 0; 346 passed, 0 failed, 0 ignored; finished in 763.97s. This ran on Windows, so the `#[cfg(unix)]` test was not compiled or executed here. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **PASS**, exit 0; finished in 44.32s. |
| `cargo metadata --no-deps --format-version 1` | **PASS**, exit 0; reports 13 packages, 13 workspace members, all package versions `0.32.0`, and edition `2024`. |
| `git diff --check` | **PASS**, exit 0. |

Final invariant audit: the workspace remains at 13 packages and 13 members,
version `0.32.0`, edition `2024`. No Cargo manifest or lockfile is changed;
there is no external dependency, checksum, feature, or workspace-membership
drift. The source diff is limited to the Unix test in
`crates/rah-tools/src/repository_boundary.rs`; production Rust and authority
implementation are unchanged. HostExplicit remains exactly 11,
`repo.list` remains read-only `RepositoryObservation`, timeout policy is
unchanged, and no Tauri permission-generation work was added. The Windows
focused test suite does not establish Unix compilation or execution; new
exact-head Linux CI remains required.

At the end of local validation, no files were staged and no commit, push, or
new CI run had been started. Those steps are recorded in the final task
closeout; the original Task 423C formatting stop remains preserved above.

## Task 423C2 continuation — repository-list Git discovery portability

Task 423C2 continues this stabilization chain without changing the earlier
Task 423C formatting STOP or Task 423C1 validation evidence. The starting
checkpoint was clean at `756377e4a2f664a67d4d8e48b3393400b00b199e`, equal to
`origin/master`; the index was empty.

Task 423C1's new exact-head Linux CI run `36276645054` tested that SHA on
Ubuntu 24.04. Formatting, workspace check, and the corrected Unix
repository-boundary test passed. Workspace tests failed (334 passed, 7
failed, 0 ignored), so lint was skipped and the workflow failed. The exact
failing tests were:

- `repository_list::tests::deep_tracked_path_lists_in_ordinary_and_linked_worktrees`
- `repository_list::tests::definition_is_closed_and_execute_bound`
- `repository_list::tests::direct_child_saturation_is_deterministic`
- `repository_list::tests::nested_repository_fails_without_partial_entries_and_observation_does_not_mutate`
- `repository_list::tests::request_validation_and_target_failures_are_closed`
- `repository_list::tests::root_nested_direct_projection_and_visibility_are_bounded`
- `repository_list::tests::tracked_binary_file_is_listed_structurally_without_disclosing_contents`

All seven failed at `crates/rah-tools/src/repository_list.rs:312:72` with
`Os { code: 2, kind: NotFound, message: "No such file or directory" }`.
The unconditional test-helper invocation of `where.exe git.exe` is the
bounded root cause: `where.exe` is unavailable on Ubuntu, so fixture
construction failed before repository-list behavior was exercised. This is
not evidence of a production runtime, Git-layout, timeout, authority, or
repository-observation semantic defect. Run `36276645054` remains historical
evidence and was not rerun.

`crates/rah-tools/src/repository_search.rs` already used the required
cross-platform `native_git()` precedent: Windows `where.exe git.exe`,
non-Windows `which git`, successful exit required, first returned path, and
canonicalization. Task 423C2 made only `repository_list.rs`'s test helper
match that code. No production Rust, test cases/assertions, enabled-test
attributes, Git command vectors, authority, timeout, dependency, or Cargo
metadata changed. All seven CI-failing tests remain enabled without ignore
or platform cfg attributes; the existing boundary test remains enabled on
Unix.

### Task 423C2 local validation

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS, exit 0. |
| `cargo check --workspace` | PASS, exit 0. |
| `cargo test -p rah-tools repository_list::tests:: -- --test-threads=1` | PASS, 8 passed, 0 failed, 0 ignored, 338 filtered. |
| `cargo test -p rah-tools --lib -- --test-threads=1` | PASS, 346 passed, 0 failed, 0 ignored. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS, exit 0. |
| `cargo metadata --no-deps --format-version 1` | PASS; 13 packages and members, version `0.32.0`, edition `2024`. |
| `git diff --check` | PASS, exit 0. |

Invariant audit found no manifest or lockfile diff: 13 packages and members,
all version `0.32.0`, edition `2024`; dependency/checksum, feature, and
workspace membership remain unchanged. The Rust diff is restricted to the
test helper. Production `RepositoryListTool` and `RepositoryObserver`,
read-only `RepositoryObservation` classification, `HostExplicit` count of
11, authority, timeout policy, listing bounds, path validation, and command
vectors are unchanged. No Tauri permission work was included. Windows local
validation cannot establish the `which git` branch; exact-head Linux CI is
mandatory.

Task 423C2's initial push attempt failed authentication at the configured
private HTTP push endpoint; this remains historical evidence. Recovery pushed
`e970660f0d9800e9a6435ccb23b81c7a0224e165` directly to the confirmed GitHub
repository without changing remote configuration. Exact-head Linux CI run
`36284890321` (workflow `CI`, event `push`) tested that SHA and passed
formatting, workspace check, workspace tests, lint, and overall. The corrected
Unix test `repository_boundary::tests::existing_symlink_ancestor_rejects_all_missing_descendant_depths`
passed, and all seven previously failing `repository_list::tests` passed.
Task 423C2 stabilization is complete; Task 423D is next. No publication
occurred and Task 424 has not started.

### Task 423C2 initial push attempt — historical authentication failure

The initial attempt to push commit `e970660f0d9800e9a6435ccb23b81c7a0224e165`
used the configured private HTTP push endpoint and failed with:

```text
remote: Failed to authenticate user
fatal: Authentication failed for 'http://192.168.1.253:4000/spider2449/rust-agent-harness.git/'
```

This failure was recovered by a direct normal fast-forward push to the
confirmed canonical GitHub URL. The exact-head GitHub CI run and its results
are recorded above; the initial failure is retained here as historical
transport evidence, not the current task status.

## Task 423D reconciliation

Task 423D preserves this task's formatting STOP and the later Task 423C1 continuation evidence. Exact-head CI `36276645054` tested `756377e4a2f664a67d4d8e48b3393400b00b199e`: the corrected Unix boundary test passed, but workspace tests failed because seven `repository_list` fixtures invoked Windows-only `where.exe git.exe` on Ubuntu. This historical failure was corrected only in Task 423C2's test Git-discovery helper. Exact-head CI `36284890321` then passed formatting, workspace check, workspace tests, and lint for `e970660f0d9800e9a6435ccb23b81c7a0224e165`; the Unix boundary test and all seven repository-list tests passed. Neither failed run was retried or removed from the record.

Task 423D makes documentation-only release-record changes. It does not alter the source correction, test scope, timeout policy, authority, or permissions. The readiness commit and its own exact-head CI are a separate final gate; the Task 423D completion report records their resulting SHA and run. No tag or publication is part of Task 423D.
