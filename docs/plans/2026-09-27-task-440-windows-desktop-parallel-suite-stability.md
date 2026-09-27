# Task 440 — Windows Desktop parallel suite stability

Status: STOPPED / NOT CERTIFIED after the first required post-correction Desktop gate failed. Task 438 and Task 439 remain paused and uncertified.

## Base and preserved evidence

- Clean isolated worktree: `deba6a10d65ce5db9842a77db3420a89cc0201ec`.
- Provider helper executables were built in `target-task440/debug` before testing.
- The saved Task 439 run was captured without panic output. It reports four failures, including `task_321_i_real_unstage_reservation_rejects_connect_admission`, one more than the three in the task request. It names only `task_320_loser_after_winner_preserves_winner_state` as started but never completed. A second outstanding name cannot be recovered from that log.
- On the clean base, all four failed tests passed 3/3 independently. The named outstanding test passed 3/3 independently.

## Saved log interpretation

| Test | First failing operation in saved log | Error category | Git/process? | Async barrier? | Final state |
| --- | --- | --- | --- | --- | --- |
| `task_320_successful_switch_invalidates_old_commit_and_workflow_state` | Not recorded; captured panic text is absent | Unknown | Fixture and observer paths | No installed test barrier | Failed |
| `task_321_c_authorization_preparation_cannot_rearm_after_activation` | Not recorded; captured panic text is absent | Unknown | Fixture and observer paths | Authorization hook installed later in test | Failed |
| `repository_snapshot_matrix_isolated_repositories_and_replacements` | Not recorded; captured panic text is absent | Unknown | Git observer | No test barrier | Failed |
| `task_321_i_real_unstage_reservation_rejects_connect_admission` | Not recorded; captured panic text is absent | Unknown | Real Git fixture | No test barrier | Failed |
| `task_320_loser_after_winner_preserves_winner_state` | No failure line in saved log | Outstanding | Git fixture and observer | Activation barrier | Outstanding |

The last saved output is a successful `task_359_linked_worktrees_use_one_revalidated_active_composition` result, followed by Cargo's abnormal-exit message after the run was stopped. The log does not contain a completed test summary.

## Clean-base normal parallel reproduction

The uncaptured Task 440 run reproduced failures:

- `repository_snapshot_matrix_isolated_repositories_and_replacements`: `desktop_repository_snapshot(&second_a).await.is_ok()` was false at `main_tests.rs:2259`.
- `task_321_c_authorization_preparation_cannot_rearm_after_activation`: `authorize_test_commit` panicked at `main_tests.rs:842` with `StagedDiffExecution`. This occurs before that test installs its authorization barrier.
- `task_320_loser_after_winner_preserves_winner_state`: `authorize_test_commit` panicked at `main_tests.rs:842` with `StagedDiffExecution` while activation C was already blocked at the activation publication barrier. The panic occurs before `release_activation_barrier`. The test did not report completion, and the Desktop test binary remained alive with no Git or provider child in the later process snapshot. The bounded run was stopped.

`StagedDiffExecution` collapses underlying observer errors. The log does not prove timeout, process exhaustion, or a specific Git child error. No timeout value is changed.

## Root cause and correction scope

The hang mechanism is **A — DESKTOP TEST BARRIER CLEANUP DEFECT**. A parent test can unwind after a spawned task reaches a blocking `std::sync::Barrier` and before releasing it. The blocked runtime worker prevents clean test termination. The clean-base reproduction proves this sequence in `task_320_loser_after_winner_preserves_winner_state`.

The test-only correction replaces the five publication hook barriers with a release gate and a receiver guard whose destructor releases the worker during unwinding. It also covers the local host-preparation barrier. The unrelated observer failures remain separately unlocalized pending validation; no production behavior, repository timeout, authority, or product capability is changed.

## Other surfaces audited

- Repository leases are keyed by normalized repository root. Distinct temporary roots get distinct leases; the registry stores `Weak` entries, which cannot retain a lock. No lease deadlock was shown.
- Desktop Git fixtures use timestamp plus atomic sequence directory names. Their destructor ignores cleanup errors; the failures above do not establish a cleanup collision.
- Remembered-workspace and desktop-preference fault state use process-global test hooks. The failed tests do not use those hooks, so they are not assigned as this failure's cause.
- Startup counters and location-hint access counters are process-global test instrumentation. They are not read by the failing repository tests. The live-only environment mutation test is ignored in normal runs.
- The process snapshot during the clean-base stall showed a live Desktop test binary and Cargo parents, with no active Git or provider child. This supports the blocked worker explanation for the hang, not the source of the earlier observer failure.

## Validation boundary

The exact `task_320_loser_after_winner_preserves_winner_state`, `task_321_c_authorization_preparation_cannot_rearm_after_activation`, and panic-unwind regression tests passed after the correction. `cargo fmt --check` and `git diff --check` passed after formatting.

The first required normal parallel Desktop gate completed with **319 passed, 2 failed, 18 ignored** in 249.38 seconds. Both failures were in the separate, uncorrected remembered-workspace fault-state area on the clean committed base. `coordination_failure_and_delete_are_bounded_and_isolated` expected `Err(StorageFailure)` but got `Ok(())`; `save_reparses_staged_bytes_and_preserves_previous_file_on_failures` then encountered a poisoned test mutex. The Task 439 correction remains isolated in its own worktree and was not imported here. All three originally named repository failures, the extra unstage failure, and the previously outstanding activation test passed in this gate.

This required gate failure stops Task 440 validation. No second Desktop run, workspace run, commit, push, or CI claim follows. The test-only correction remains uncommitted in the Task 440 worktree. The underlying `StagedDiffExecution` failure observed before the correction remains unlocalized; the successful post-correction repository tests in one run do not prove it resolved.
