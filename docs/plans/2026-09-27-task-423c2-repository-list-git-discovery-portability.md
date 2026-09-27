# Task 423C2 — Repository List Git Discovery Portability

Date: 2026-09-27
Status: **PASS - PUSH AND EXACT-HEAD CI PASSED**
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

Commit: `e970660f0d9800e9a6435ccb23b81c7a0224e165`
Subject: `test: make repository list Git discovery portable`
Files: the four intentional paths listed in the staged-content audit above.

The initial configured push attempt failed authentication at the private HTTP
push endpoint. The original output was:

```text
remote: Failed to authenticate user
fatal: Authentication failed for 'http://192.168.1.253:4000/spider2449/rust-agent-harness.git/'
```

This remains historical evidence. The GitHub CLI was already
authenticated as `spider2449`; the configured GitHub fetch URL identified
`spider2449/rust-agent-harness`, and `origin/master` was the expected parent.
Without changing remote configuration, the existing commit was pushed directly
to `https://github.com/spider2449/rust-agent-harness.git` as a normal
fast-forward. The recovery push succeeded for exactly
`e970660f0d9800e9a6435ccb23b81c7a0224e165`. A subsequent fetch verified both
local `HEAD` and GitHub `master` at that SHA.

Exact-head GitHub Linux CI:

```text
Run:      36284890321
Workflow: CI
Event:    push
SHA:      e970660f0d9800e9a6435ccb23b81c7a0224e165
Format:   PASS
Check:    PASS
Tests:    PASS
Lint:     PASS
Overall:  PASS
```

The workspace test log confirms all seven previously failing
`repository_list::tests` passed. The corrected Unix test
`repository_boundary::tests::existing_symlink_ancestor_rejects_all_missing_descendant_depths`
also passed. The initial authentication failure remains recorded above; the
recovery push and this exact-head CI run establish Task 423C2 stabilization.

## Publication and next task

No tag, GitHub Release, or publication occurred. Task 423D and Task 424 have
not started. Task 423C2 technical stabilization is complete; Task 423D is the
next task and will finalize v0.32.0 release readiness after the CI corrections.

## Task 423D final-readiness reconciliation

Task 423D retained both failed historical exact-head checkpoints:

- `0301a52095b1b7c3717a6497f4f2dd042b184bae` / CI `36275076398` failed at Unix test compilation because the fixture's local `PathBuf` shadowed `root(...)`.
- `756377e4a2f664a67d4d8e48b3393400b00b199e` / CI `36276645054` passed the corrected Unix boundary test but failed seven Linux `repository_list` fixtures due to Windows-only Git discovery; 334 tests passed and 7 failed.

The completed correction checkpoint is `e970660f0d9800e9a6435ccb23b81c7a0224e165` / CI `36284890321` (push), with formatting, workspace check, tests, lint, and overall all passing. The seven previously failing repository-list tests and Unix boundary test passed. These results do not transfer to the later Task 423D documentation commit; new exact-head CI is required for that commit.

Task 423D's metadata/source audit found 13 packages and members, version `0.32.0`, edition `2024`, no Cargo manifest or lockfile drift since release preparation, and only the two documented test-only `.rs` portability fixes since that preparation commit. The audit found no production behavior, authority, permission, or timeout-policy change. `repo.list` remains read-only `RepositoryObservation`; HostExplicit remains exactly 11; the unrelated Tauri permission-generation debt remains out of scope.

The Task 423D pre-commit status is `LOCAL RELEASE RECORDS RECONCILED`. Final readiness depends on the documentation-only commit being pushed and passing its own exact-head CI. Its SHA and CI result are given in the Task 423D completion report. No tag, GitHub Release, or publication occurred; Task 424 remains reserved until readiness is established.
