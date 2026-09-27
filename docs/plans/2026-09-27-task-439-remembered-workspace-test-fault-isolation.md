# Task 439 — Remembered workspace test fault isolation

Status: **STOP — not certified, not committed.** Task 438 remains paused and uncommitted.

## Cause and audit

The bounded four-thread experiment for Task 438 exposed a remembered-workspace fault-injection failure. `remembered_workspace.rs` uses process-wide `TEST_STATE: OnceLock<Mutex<TestState>>` and `TEST_LOCK: OnceLock<Mutex<()>>`. `TestState` holds one `fault` and an `operations` vector. `set_test_fault()` replaces that fault and clears the operations vector, so tests that set or inspect this state must hold the same test lock throughout their sensitive interval.

Repository-wide search for `set_test_fault(`, `clear_test_state(`, and `test_operations(` found **four remembered-workspace setter call sites**: three in `remembered_workspace.rs` and one in `main_tests.rs`. The save/reparse and coordination tests already held `TEST_LOCK`; the reparse-ancestor test and `task_333_failed_catalog_save_preserves_published_and_durable_state` did not. The remembered-workspace test-directory destructor also called `clear_test_state()` from every test using that directory, including tests without a fault; this unguarded global clear could interfere with a guarded fault test.

The narrow test-only correction gives the two unguarded fault tests the existing lock, exposes that lock within the crate's test build for `main_tests.rs`, and removes the destructor's global clear. Every remembered-workspace setter, explicit clear, and operation assertion is now within a guarded fault test. The test-only change does not alter production file format, persistence, authority, filesystem behavior, error mapping, coordination, or timeouts. No security or authority boundary changes; HostExplicit remains 11, v0.33 selects no product capability, and the version remains 0.32.0.

## Validation and stop evidence

- The three requested focused remembered-workspace fault tests: **PASS individually**.
- The additional `tests::task_333_failed_catalog_save_preserves_published_and_durable_state`: **PASS individually**.
- Desktop normal parallel run 1: **FAIL**, 317 passed, 3 failed, 18 ignored. The failures are three `provider_composition::tests::*` tests. Each reports that a provider fixture executable must be built before the focused Desktop test (`rah-plugin-echo.exe` or `rah-mcp-echo-server.exe`). Exact output: `F:\Temp\rah-task439-gates\desktop-normal-1.log`.
- Desktop normal parallel run 2: **not run** after run 1 failed.
- Four-thread diagnostic: **not run**.
- Workspace test gate: **not run**.
- Standard validation and metadata: **not run**.
- Exact-head CI: **not run**; there is no Task 439 commit or push.
- Task 438 stability gate and live certification: **not resumed**.

The failure is a fixture prerequisite in this new worktree, not evidence of a remembered-workspace production semantics defect. Required Task 439 gates are incomplete. The authoritative Task 438 tree remains untouched, with its tracked diff separately saved at `F:\Temp\rah-task438-wip-pre-task439.patch` and its untracked WIP left in place.

## Task 439R fixture recovery and restarted gates

Desktop normal run 1 initially failed because provider-composition tests require sibling-package fixture executables that had not been built in the fresh isolated worktree. The tests explicitly require `rah-mcp-echo-server` and `rah-plugin-echo` under the selected test target directory. This was **TEST INVOCATION / FIXTURE PREREQUISITE NOT SATISFIED**. No provider production/test source correction was required. The fixtures were built explicitly before restarting the Task 439 gates.

The isolated worktree remained at `deba6a10d65ce5db9842a77db3420a89cc0201ec`. Both `CARGO_TARGET_DIR` and `RAH_TEST_TARGET_DIR` were set to its `target-task439` directory for the remaining build and test commands. `cargo build -p rah-tools-mcp --bin rah-mcp-echo-server -p rah-tools-plugin --bin rah-plugin-echo` passed, and both `.exe` files were verified as regular files in `target-task439/debug`.

The three exact provider-composition tests from the initial failed log each **PASS individually** after fixture preparation: `activation_rereads_current_source_and_keeps_admitted_snapshot_stable`, `changed_source_invalid_or_missing_fails_before_provider_publication`, and `mixed_external_providers_are_fresh_composed_merged_usable_and_reaped`.

The repository-wide audit still has four remembered-workspace `set_test_fault()` call sites. All four fault-sensitive tests **PASS individually** on the restarted gate: `save_reparses_staged_bytes_and_preserves_previous_file_on_failures`, `reparse_ancestor_rejection_has_no_catalog_or_coordination_mutation`, `coordination_failure_and_delete_are_bounded_and_isolated`, and `task_333_failed_catalog_save_preserves_published_and_durable_state`. They now share `TEST_LOCK` while using process-global `TEST_STATE`; the correction addresses the missing serialization and destructor interference described above.

- Desktop normal restarted run 1: **PASS**, 320 passed, 0 failed, 18 ignored. Log: `F:\Temp\rah-task439-gates\desktop-normal-restart-1.log`.
- Desktop normal restarted run 2: **FAIL / incomplete**. The log reported `tests::task_320_successful_switch_invalidates_old_commit_and_workflow_state`, `tests::task_321_c_authorization_preparation_cannot_rearm_after_activation`, and `tests::repository_snapshot_matrix_isolated_repositories_and_replacements` as `FAILED`. It stopped advancing with 315 `ok` results and two further tests outstanding; after about 14 minutes, the still-running test process was stopped to preserve the failed gate. Cargo reported an abnormal process exit. Exact output: `F:\Temp\rah-task439-gates\desktop-normal-restart-2.log`.
- Four-thread run, both workspace runs, standard validation, metadata, commit, push, exact-head CI, and Task 438 resumption: **not run**, because restarted Desktop run 2 failed.

Task 439 remains **STOPPED / NOT CERTIFIED**. The restarted failure is distinct from the provider fixture prerequisite and has not been classified as a remembered-workspace regression. Preserve the test-only correction and the failed gate evidence for separate diagnosis.
