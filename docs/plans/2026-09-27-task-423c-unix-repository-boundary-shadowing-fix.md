# Task 423C — Unix Repository Boundary Test Shadowing Fix

Date: 2026-09-27
Status: **LOCAL VALIDATION PASS — NEW EXACT-HEAD LINUX CI PENDING**
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
