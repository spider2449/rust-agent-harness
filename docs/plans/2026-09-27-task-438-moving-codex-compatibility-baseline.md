# Task 438 — Moving Codex compatibility baseline

Date: 2026-09-27

Starting HEAD and origin/master: `deba6a10d65ce5db9842a77db3420a89cc0201ec`.
Starting worktree: clean.

## Direct control

The exact saved Windows x86_64 0.157.1 baseline reported `codex-cli 0.157.1`.
Its independently verified SHA-256 is
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
The formal direct `gpt-6-luna` no-Tool control returned the exact
`RAH438_CODEX_01571_OK` marker and completed normally. The same-account
0.149.0 direct control failed in Task 437. The current host default was
`gpt-6-sol`, so the formal run explicitly selected `gpt-6-luna` without
changing the host configuration.

The 0.157.1 baseline directory predated this corrected run. Its presence is
not Task 438 certification; its manifest and artifact integrity were checked
independently. Both 0.149.0 and 0.157.1 baselines are retained.

## App-server schema audit

Exact 0.157.1 `app-server generate-json-schema --experimental` output was
generated locally. The captured required 0.149.0 contract paths and required
fields all remain present: initialize; thread start/resume; turn start,
interrupt, and completion; streamed message deltas; thread/turn IDs; and
dynamic Tool call parameters and result. The current schema still exposes
`completed`, `failed`, and `interrupted` turn states, `never` approval policy,
read-only sandbox settings, and dynamic Tool `contentItems`/`success` response.
Additional optional schema fields are additive compatible. The adapter keeps
correlated request IDs and explicit JSON-RPC error handling. No breaking
change requiring a new public adapter contract was identified in these fields.

## Windows test observation

The first formal Windows workspace test run failed once in
`repository_file_info::tests::linked_file_info_reads_the_selected_worktree_file_and_head`
during its final bounded status observation.

No related source differences existed from the Task 438 starting checkpoint.

The exact focused test subsequently passed six of six runs at roughly
2.5 seconds per run, and the subsequent normal parallel workspace test passed.

The event remains recorded as an unresolved transient/load-sensitive Windows
observation and is not attributed to the Codex 0.157.1 compatibility change.

## Completion gate

Task 438 is not certified. After changing the deterministic Desktop fake Codex
version to 0.157.1, the ordinary parallel `cargo test --workspace` run passed
Desktop (320 passed, 18 ignored) but failed in the `rah-runtime-codex` library:

- `bridge_tests::trusted_profile_composed_observers_advertise_canonical_schemas_and_observe_read_only`
  received `RAH tool execution failed` on a composed observer call.
- `bridge_tests::composed_observer_deduplication_is_call_identity_not_input_memoization`
  observed fewer successful calls than expected.

The first of these also failed in one exact isolated test run. The second
passed in one exact isolated run. An earlier focused parallel library run also
failed in these and two other composed repository observer tests. No Task 438
source change was made in the repository observer or Tool implementation.
The workspace run stopped at `rah-runtime-codex` and did not execute
`rah-tools`; it provides no new result for the specified linked file-info test.
The observer failure is unresolved and no full-suite PASS is claimed.

`cargo fmt --check`, `cargo check --workspace`, clippy with all targets and
features, Tauri permission inventory, frontend syntax and authority tests,
`git diff --check`, and metadata all passed. Metadata remains 13 packages,
13 workspace members, version 0.32.0, edition 2024.

The second direct baseline control, production Desktop neutral and repository
context certification, model-selected Tool observation, scoped commit, push,
and exact-head CI remain pending behind the failed workspace test gate.

## 2026-09-27 bounded observer comparison and workspace retry

The uncommitted Task 438 tree remained at
`deba6a10d65ce5db9842a77db3420a89cc0201ec`. Its tracked binary diff was
saved for recovery only at `F:\Temp\rah-task438-wip.patch` (82,208 bytes).
The three untracked Task 438 files remain in the working tree and are not
contained in that tracked diff backup.

Both failures were `rah-runtime-codex` library tests in `bridge_tests`:

- `bridge_tests::trusted_profile_composed_observers_advertise_canonical_schemas_and_observe_read_only`:
  the earlier workspace run reported `RAH tool execution failed` during a
  composed repository observer call. The earlier exact isolated run also
  failed. The full earlier error chain was not retained in this report.
- `bridge_tests::composed_observer_deduplication_is_call_identity_not_input_memoization`:
  the earlier workspace run asserted fewer successful observer calls than
  expected. Its earlier exact isolated run passed. The full earlier assertion
  values were not retained in this report.

Each was run exactly three times in the dirty tree with
`cargo test -p rah-runtime-codex --lib <exact test name> -- --exact --nocapture --test-threads=1`
and `RUST_BACKTRACE=1`: both were **3/3 PASS**. The same commands, environment,
and toolchain at the detached clean base were also **3/3 PASS** for each test.
The first isolated dirty run is included in the three attempts. Logs are at
`F:\Temp\rah-task438-dirty-*.log` and `F:\Temp\rah-task438-clean-*.log`.
The clean comparison worktree was removed normally. This matrix does not
reproduce a Task 438 regression; the earlier failures remain consistent with
workspace interaction or transient load, without a proven cause. No observer
source or fixture was changed.

The ensuing normal parallel `cargo test --workspace` **FAILED** in
`rah-desktop`: 304 passed, 16 failed, 18 ignored, after 284.98 seconds.
Failures were concentrated in repository snapshot, index action, and commit
review tests. Several reported `StagedDiffExecution` or repository observation
total timeout; others could not find the expected action or review. The exact
names and panic output are preserved in
`F:\Temp\rah-task438-workspace-retry.log`. The run stopped at Desktop, so
there is no new `rah-runtime-codex` workspace result. This is a new blocking
workspace result, not a PASS or a reason to rewrite the earlier observer
failure history. No correction was made. Task 438 remains **NOT CERTIFIED**;
the full deterministic gate and live certification were not resumed.

The 16 failing `rah-desktop` test names were:

- `tests::disconnect_revokes_pending_authorization_and_never_restores_old_review`
- `tests::observed_single_target_stage_and_unstage_refresh_and_consume_selectors`
- `tests::repository_snapshot_matrix_isolated_repositories_and_replacements`
- `tests::review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors`
- `tests::same_size_target_change_rejects_displayed_stage_action`
- `tests::task349_close_clears_pending_no_effect_commit_authorization`
- `tests::task349_close_preserves_runtime_model_and_repository_effect_owners`
- `tests::task349_close_wins_against_commit_authorization_publication`
- `tests::task_320_stale_target_after_final_preparation_preserves_active_state`
- `tests::task_320_successful_switch_invalidates_old_commit_and_workflow_state`
- `tests::task_321_c_authorization_preparation_cannot_rearm_after_activation`
- `tests::task_321_e_stage_reservation_wins_over_activation`
- `tests::task_321_e_unstage_reservation_wins_over_activation`
- `tests::task_321_i_real_stage_reservation_rejects_real_connect_admission`
- `tests::task_321_i_real_stage_reservation_rejects_real_connect_publication`
- `tests::task_321_i_real_unstage_reservation_rejects_connect_admission`

## Task 438D Desktop failure investigation (27 September 2026)

The authoritative tree remained at `deba6a10d65ce5db9842a77db3420a89cc0201ec`,
with the Task 438 WIP uncommitted and the recovery patch present. The saved
workspace panic log gives the following first observed failures. `StagedDiffExecution`
is the Desktop stage that executes the staged-diff observer; a missing review or
action can follow an unavailable observation but the saved panic alone does not
prove its lower-level error. All names below retain their `tests::` prefix.

| Exact test name | Panic/error | First failing operation | Category/stage |
| --- | --- | --- | --- |
| `tests::disconnect_revokes_pending_authorization_and_never_restores_old_review` | `redacted snapshot observation: StagedDiffExecution` | Snapshot staged diff | Observer execution |
| `tests::observed_single_target_stage_and_unstage_refresh_and_consume_selectors` | `snapshot.review` was not `ReviewAvailable` | Review presentation assertion | Missing review |
| `tests::repository_snapshot_matrix_isolated_repositories_and_replacements` | `A snapshot succeeds: StagedDiffExecution` | Snapshot staged diff | Observer execution |
| `tests::review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors` | index 0 of empty list | Review/action list lookup | Missing action |
| `tests::same_size_target_change_rejects_displayed_stage_action` | `stage action exists` | Stage action lookup | Missing action |
| `tests::task349_close_clears_pending_no_effect_commit_authorization` | `repository observation exceeded its total timeout` | Reviewed snapshot observation | Explicit aggregate timeout |
| `tests::task349_close_preserves_runtime_model_and_repository_effect_owners` | `modified repository snapshot should complete: StagedDiffExecution` | Snapshot staged diff | Observer execution |
| `tests::task349_close_wins_against_commit_authorization_publication` | `staged review should be observed: StagedDiffExecution` | Reviewed snapshot staged diff | Observer execution |
| `tests::task_320_stale_target_after_final_preparation_preserves_active_state` | `commit review should be available` | Review presentation assertion | Missing review |
| `tests::task_320_successful_switch_invalidates_old_commit_and_workflow_state` | `commit review should be available` | Review presentation assertion | Missing review |
| `tests::task_321_c_authorization_preparation_cannot_rearm_after_activation` | `commit review should be available` | Review presentation assertion | Missing review |
| `tests::task_321_e_stage_reservation_wins_over_activation` | `A should expose a Stage action` | Stage action lookup | Missing action |
| `tests::task_321_e_unstage_reservation_wins_over_activation` | `A should expose an Unstage action` | Unstage action lookup | Missing action |
| `tests::task_321_i_real_stage_reservation_rejects_real_connect_admission` | `workflow should expose the requested real index action` | Stage action lookup | Missing action |
| `tests::task_321_i_real_stage_reservation_rejects_real_connect_publication` | `workflow should expose the requested real index action` | Stage action lookup | Missing action |
| `tests::task_321_i_real_unstage_reservation_rejects_connect_admission` | `workflow should expose the requested real index action` | Unstage action lookup | Missing action |

The three actual panic signatures are direct staged-diff execution failure (4),
explicit aggregate timeout (1), and unavailable review/index action or an
assertion on one (11). The latter group does not expose a lower-level cause.
There was no saved evidence of a fixture Git command exit, stale/currentness
rejection, cleanup error, or lock error in these 16 panics.

`cargo test -p rah-desktop --lib` cannot execute because this package has no
library target. The equivalent test target is `--bin rah-desktop`, run without
`--test-threads=1`. The dirty Task 438 binary run: **315 passed, 5 failed,
18 ignored** in 225.35 seconds. All 16 original failures passed. Its five new
failures were `authorize_then_refresh_revokes_pending_and_rotates_review_selector_without_commit`
(missing fresh review), `desktop_registry_commit_requires_its_paired_control_and_is_one_shot`
(explicit aggregate timeout), and the three
`deletion_hostexplicit_{success_dispatches_once_and_invalidates_commit_review,rejects_stale_before_started_for_git_and_desktop_drift,post_started_failures_are_uncertain_without_replay}`
(staged diff or review unavailable). This is a different suite failure set,
with overlapping observation/review signatures.

A detached clean-base worktree ran the same Desktop binary test under ordinary
parallel execution: **315 passed, 5 failed, 18 ignored** in 217.22 seconds.
Its five failures did not overlap the repository-observation set: three
`provider_composition` tests required helper executables absent from this
package-only build, and two `remembered_workspace` tests reported a storage
assertion failure and subsequent poisoned mutex. The missing helper executables
limit that crate-level comparison; they are a known prerequisite of those tests,
not evidence about repository observation. The clean base's repository tests
passed in this one run.

Three representatives covered the explicit timeout, staged-diff execution,
and unavailable index-action stages. Each used `RUST_BACKTRACE=1`, `--exact`,
`--nocapture`, and `--test-threads=1`, exactly three times in each tree:

| Representative (`tests::` prefix) | Dirty Task 438 | Clean base | Interpretation |
| --- | --- | --- | --- |
| `task349_close_clears_pending_no_effect_commit_authorization` | 3/3 PASS | 3/3 PASS | Timeout was suite-context dependent |
| `repository_snapshot_matrix_isolated_repositories_and_replacements` | 3/3 PASS | 3/3 PASS | Staged-diff failure was suite-context dependent |
| `task_321_i_real_stage_reservation_rejects_real_connect_admission` | 3/3 PASS | 3/3 PASS | Missing action was suite-context dependent |

The Task 438 Desktop source diff changes Codex baseline resolution/version
presentation and associated test version constants. The repository snapshot
path executes repository status, worktree diff, and staged diff or commit review
through `DesktopRepository`; it does not resolve or start the Codex executable.
The failing repository fixtures use temporary Git repositories and native Git.
The Task 438 changed fake Codex executable is used by runtime connection tests,
not by this snapshot path. A direct Task 438 dependency for these observation
failures was not found.

A Desktop source search found no ordinary test that mutates process current
directory, `RAH_CODEX_EXECUTABLE`, `CODEX_BASELINE_HOME`, or `PATH`. The sole
`RAH_GIT_EXECUTABLE` mutation is in an ignored host-only Git-discovery probe,
so it did not run in either normal crate test. Repository fixture names combine
time and a process-local atomic sequence; this search did not identify a shared
temp-path collision. These negative checks do not establish the scheduler or
timeout mechanism.

**Causal disposition:** the changing failure set and isolated 3/3 passes
establish suite-context instability; the explicit timeout is one observed
symptom. They do not establish whether process scheduling, a shared state race,
or aggregate-budget composition is the root cause. No timeout limit, observer
implementation, fixture, or production bridge was changed. In particular,
the clean run's repository PASS does not prove Task 438 caused the dirty failures,
and its unrelated failures prevent a clean Desktop gate. Task 438 remains
**NOT CERTIFIED**. Normal parallel Desktop and workspace PASS gates are still
required before the second saved 0.157.1 direct control and live sequence.
Logs: `F:\Temp\rah-task438d-{dirty,clean}-desktop.log` and the nine exact-test
logs per tree under `F:\Temp\rah-task438d-{dirty,clean}-tests-*.log`.

## Task 438E Windows Desktop timing investigation (27 September 2026)

The starting HEAD remained `deba6a10d65ce5db9842a77db3420a89cc0201ec`;
the uncommitted Task 438 WIP and 82,208-byte recovery patch were present.
The exact Desktop binary target is `cargo test -p rah-desktop --bin rah-desktop`.
No prior Task 438D comparison matrix was repeated.

Source timeout composition: `RepositoryStatusTool` makes one status command
after eight Git-layout probes, with a 10-second operation/child ceiling.
`RepositoryDiffTool` makes raw, numstat, and patch commands, each preceded by
eight probes, within one 15-second operation budget. `RepositoryDiffStagedTool`
adds pre- and post-HEAD commands, so its five commands and 40 repeated probes
share one 15-second aggregate `Instant`; each HEAD child has a 5-second ceiling
further capped by the aggregate remainder. Worktree diff has no separate
aggregate passed into Git-layout validation, but its child allowances still
decrease from the operation start. `RepositoryFileInfoTool` shares one
5-second start across index, HEAD, optional HEAD-tree, and status (three or
four commands, each with eight probes). Each layout probe has a 5-second
ceiling, reduced by aggregate remainder when one is supplied. Desktop snapshot
sequentially executes status, worktree diff, staged diff or review generation,
then normalization; these are separate observer operation clocks. Review
failure can trigger a staged-diff fallback and make the Desktop staged stage
longer than a single 15-second observer budget.

Temporary, environment-gated `RAH_TEST_OBSERVER_TIMING_PATH` instrumentation
printed only command kinds, relative milliseconds, exit category, timeout and
overflow flags, plus Desktop stage times to local redirected logs. It printed
no repository content or paths. It was removed after the two diagnostic runs;
it is not product telemetry. The first compile attempt stopped before any test
because the temporary Debug formatting needed an internal enum derive; the
derive was added for diagnostics and removed afterward. The one ordinary
parallel timing suite then **PASSED: 320 passed, 0 failed, 18 ignored** in
238.91 seconds (`F:\Temp\rah-task438e-desktop-timing.log`).

Observed successful-run timings, in milliseconds (min / median / max):

| Command or Desktop stage | Samples | Time |
| --- | ---: | ---: |
| status child | 60 | 44 / 130 / 311 |
| worktree raw / numstat / patch children | 60 each | 40 / 130 / 361; 42 / 128 / 376; 48 / 114 / 373 |
| staged raw / numstat / patch children | 62 each | 43 / 130 / 330; 44 / 130 / 438; 43 / 134 / 485 |
| HEAD child | 124 | 40 / 115 / 387 |
| Desktop status | 60 | 428 / 1379 / 2071 |
| Desktop worktree diff | 60 | 1492 / 4223 / 5202 |
| Desktop staged diff/review | 60 | 3760 / 7200 / 31612 |
| Desktop normalization | 60 | 0 / 0 / 1 |

The smallest recorded successful child-launch allowance was 8,044 ms for
status, 9,962 ms for worktree patch, and 8,140 ms for staged patch; HEAD
remained at its 5,000 ms command ceiling. Successful validation phases were
typically about 1.2 seconds and reached 2.13 seconds. There was one expected
nonzero Git probe in a test that deliberately rejected an observation; it did
not fail the suite. No recorded child timed out or overflowed, and no aggregate
budget was exhausted in this run. The high Desktop staged-stage maximum alone
does not prove a timeout defect because review and fallback can execute in
sequence. No failing repository test was available for a per-test timing table.

The process-global audit found no ordinary Desktop test changing `PATH`,
`RAH_CODEX_EXECUTABLE`, `CODEX_BASELINE_HOME`, `LOCALAPPDATA`, `HOME`,
`USERPROFILE`, current directory, or Git environment. An ignored Git-discovery
host probe removes and later restores `RAH_GIT_EXECUTABLE`; its restoration is
not guarded against a panic inside the probe. It did not run here.
Repository fixture paths use unique process/time/atomic components. The
Desktop test infrastructure also has `OnceLock`, mutex, atomic, and thread-local
state; no demonstrated mutation of those objects correlated with repository
observer failures. A mutex by itself is not taken as proof of isolation.

The clean-base provider failures were a command-shape fixture issue:
`provider_composition::tests::fixture_program` looks directly under the
workspace `target/debug` (or `RAH_TEST_TARGET_DIR`) for `rah-plugin-echo.exe`
and `rah-mcp-echo-server.exe`. It does not use `CARGO_BIN_EXE_*` and asserts
that these binaries were built before a focused Desktop test. The clean-base
package-only target lacked them; this is separate from observation timing.

The clean-base remembered-workspace failure has a distinct test isolation
mechanism. Its injected fault is stored in a process-global `TEST_STATE`.
Two fault tests hold `TEST_LOCK`, but every `TestDirectory::drop` calls
`clear_test_state()` without taking that lock. A concurrent unrelated fixture
drop can clear an active fault. The clean-base coordination test expected
`StorageFailure` but got `Ok(())`; the next fault test encountered a poisoned
`TEST_LOCK`. This is a shared test-state race, not evidence that repository
observation uses the same storage root.

The bounded experiment used four threads, below this host's 12 reported
processors: `cargo test -p rah-desktop --bin rah-desktop -- --test-threads=4
--nocapture`. It **FAILED: 319 passed, 1 failed, 18 ignored** in 301.68
seconds (`F:\Temp\rah-task438e-desktop-four-threads.log`). The sole failure
was `remembered_workspace::tests::save_reparses_staged_bytes_and_preserves_previous_file_on_failures`:
an expected `StagedReparse` fault was absent. Repository observation tests
passed. Lower parallelism therefore did not establish a repository timing
cause and is not a certification workaround.

**Outcome D — WINDOWS DESKTOP SUITE-CONTEXT INSTABILITY REMAINS UNLOCALIZED.**
Earlier repository observation failures and explicit aggregate timeout remain
valid historical evidence. This run did not reproduce one, so it cannot prove
aggregate composition, process timeout, Git contention, or runtime starvation
as their mechanism. No timeout or production authority behavior was changed.
The separate remembered-workspace isolation defect and helper fixture contract
are recorded above; no unrelated maintenance correction or commit was mixed
into Task 438. Temporary instrumentation was removed. The normal Desktop
post-correction gate and two independent normal workspace PASS runs were not
reached, because no repository root cause or correction was proven. Task 438
remains **NOT CERTIFIED**; the second saved direct control and live sequence
remain paused. HostExplicit remains exactly 11, and v0.33 product capability
remains NONE SELECTED.
## Task 450 safe re-entry and live stop (28 September 2026)

Tasks 439–449 interrupted Task 438 to resolve maintenance blockers. Published
master `379666d0f149c035f7aa7332b7c835aaf308f647` had natural push CI
`36358219963` PASS. Task 450 preserved the original dirty WIP, created a fresh
tracked patch and untracked backup, and reapplied the WIP cleanly in a separate
worktree on that exact published master. The source dirty tree and both older
recovery patches remain untouched. See the Task 450 safe re-entry audit for
snapshot paths, hash, and diff review.

Deterministic re-entry gates passed: formatting, workspace check and test,
clippy, Tauri and frontend checks, diff check, and metadata (13 packages and
members, version 0.32.0, edition 2024). The canonical rah-tools harness passed
once. The canonical Desktop harness passed once with 321 passed, 0 failed,
18 ignored, and no watchdog timeout. Both saved Codex baselines verified; the
saved 0.157.1 executable retained SHA-256
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.

The second exact saved 0.157.1 `gpt-6-luna` direct no-Tool control passed with
the exact `RAH438_CODEX_01571_OK` marker and normal turn completion. An isolated
existing app-server live smoke passed schema validation, a `gpt-6-luna` model
turn, exact response marker, and clean shutdown.

The first production Desktop live test stopped after Connect and before Send:
its sanitized process census did not establish exactly one newly spawned saved
app-server executable. The combined assertion did not retain enough census
detail to identify whether the count or executable-identity predicate failed.
No Desktop neutral chat turn occurred, and no real Tool bridge observation was
attempted. Per the live-gate stop rule, the test was not retried or corrected.
Task 438 remains **NOT CERTIFIED**. No certification commit, push, CI, tag, or
release followed. Historical v0.32.0 Codex 0.149.0 release evidence is unchanged;
that history does not imply indefinite compatibility with newer models or
configuration.

Task 451 later audited process ownership with one separate Connect-only observation; see `docs/plans/2026-09-28-task-451-desktop-app-server-process-ownership-audit.md`. The Task 450 failure above remains historical evidence, and Task 438 remains **NOT CERTIFIED**.

## Task 452 live stop (28 September 2026)

Task 452's corrected census made one production Connect attempt after harness validation. It found one new Desktop-owned app-server that remained alive through five samples and matched the expected 0.157.1 SHA256, but the saved-path comparison returned false. The test stopped at **B — WRONG SAVED EXECUTABLE IDENTITY** before neutral Send and Tool observation. The reason for the path mismatch is not established because that attempt did not retain the executable path string. Task 450's historical failure remains unexplained, and Task 451 does not identify which old combined predicate failed. See `docs/plans/2026-09-28-task-452-desktop-live-certification-completion.md`. Task 438 remains **NOT CERTIFIED**; no certification commit, push, exact-head CI, tag, or release followed.

## Task 454 and Task 455 local revalidation (28 September 2026)

Task 453 proved Windows file-object identity for the saved executable and changed the live assertion to require one Desktop-owned candidate, matching file-object identity, and matching SHA256. Task 454's production Desktop Connect, neutral chat, and real `repo.file-info` Tool turn passed, with one inner execution and final completion. Its deterministic gate then stopped at two Clippy findings in the certification harness. That live PASS remains historical evidence; see the Task 454 plan.

Task 455 made only the two harness lint corrections and directly necessary formatting. Focused regressions, a new single production Desktop live session, full deterministic gates, and both canonical Windows package gates passed. The Desktop suite counted 323 passed, 0 failed, 20 ignored, with two structural passing tests and two ignored live tests added since Task 450. See the Task 455 plan for exact evidence. Current Codex runtime certification is complete locally; natural exact-head push CI is still required for final Task 438 completion. Historical release certification remains 0.149.0; current and preferred baseline is 0.157.1; unknown versions fail closed.