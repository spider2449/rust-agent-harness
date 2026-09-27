# Task 441 — integrated Desktop test stability

Status: **A — INTEGRATED WINDOWS TEST STABILITY RESTORED** after Task 448 re-entry. Base HEAD: `deba6a10d65ce5db9842a77db3420a89cc0201ec`. Task 438 remains paused pending exact-head push CI.

## Preserved gate chronology

- Nine focused integration invocations passed.
- Normal parallel Desktop run 1: 321 passed, 0 failed, 18 ignored.
- Normal parallel Desktop run 2: `same_size_target_change_rejects_displayed_stage_action`, `review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors`, and `repository_snapshot_matrix_isolated_repositories_and_replacements` reported FAILED. The preserved file is `F:\Temp\rah-task441-desktop-normal-2.log`.
- Run 2's log ends during test output, before panic summaries and final process exit. It contains no panic line or underlying error for the three failures. Their first failed operation, actual result, observation stage, timeout flag, exit category, and assertion-versus-operation classification are **unknown**. The source contains possible failure sites, but the log does not identify which one fired. In particular, the same-size failure cannot be assigned to stale detection, initial observation, or later index verification. The currentness check includes a SHA-256 content digest; timestamp granularity is not an evidenced cause.
- No Task 441 run 3 or subsequent stability gate has been performed.

## Task 442 bounded diagnosis

Each of the three failed tests passed 3/3 exact `--nocapture` reproductions; all nine binaries exited normally. Exact logs are under `F:\Temp\rah-task442-exact-*.log`.

One environment-controlled diagnostic normal parallel suite was run with `--nocapture`; output is `F:\Temp\rah-task442-diagnostic-normal.log`. It finished 318 passed, 3 failed, 18 ignored. All three Task 441 repository tests passed. The three failures were provider-composition tests whose helper executables were absent from the hardcoded `target/debug` location while this invocation used `target-task441`; they are a test setup prerequisite mismatch, not a repository observation result. This diagnostic run is not a passing Desktop gate.

The diagnostic run measured 291 native `where.exe git.exe` fixture lookups (64/135/445 ms min/median/max; sampled concurrent peak 4), 755 fixture Git subprocesses (27/270/3129 ms), 10,921 repository validation Git children (38/141/1055 ms), and 550 observer children (41/134/689 ms). Observer child timeout and overflow flags were all false; no observer execution error was logged. The single nonzero validation exit came from the deliberate invalid-executable observer-classification test, not one of the three repository tests.

The observer child launch budget was at least 5,000 ms in this run. Status had 60 commands with minimum remaining budget 7,717 ms; worktree diff had 180 with minimum 9,048 ms; staged diff had 186 with minimum 7,172 ms. Validation before observer commands took 377/1453/2615 ms min/median/max. Instrumentation adds log and process pressure, so these distributions are diagnostic observations, not an uninstrumented performance guarantee.

Task 442 outcome: **E — RESIDUAL DESKTOP SUITE INSTABILITY REMAINS UNLOCALIZED**. No third defect or correction is proven. No timeout change, cache, or broad serialization was retained. The Task 441 integrated stability sequence cannot resume until its required passing gates are performed. No commit, push, tag, or release is authorized by this report.

## Task 443 canonical gate and current stop

Task 443 added `scripts/windows-desktop-test-gate.ps1` and established an operational, self-contained Windows Desktop gate: three ordinary-parallel Desktop runs each passed 321/0/18. See `docs/plans/2026-09-27-task-443-windows-desktop-deterministic-gate-harness.md` for independent logs, exact timings, target/fixture contract, and harness validation. The older truncated run-2 log remains historical evidence but does not establish a root cause.

The first subsequent ordinary-parallel `cargo test --workspace` failed in three `rah-runtime-codex` composed observer bridge tests (82 passed, 3 failed, 1 ignored for that library). Complete assertions are preserved in `F:\Temp\rah-task443-workspace-1.log`; the underlying operation failure is not exposed there. The second workspace run and standard deterministic validation were not performed after this required gate failed. Task 441 remains **STOPPED / NOT CERTIFIED**. No commit or push is authorized.

## Task 444 composed observer diagnosis

The exact three first-workspace failures were `bridge_tests::trusted_profile_composed_observers_advertise_canonical_schemas_and_observe_read_only`, `bridge_tests::composed_observer_deduplication_is_call_identity_not_input_memoization`, and `bridge_tests::composed_repository_observers_verify_multi_patch_post_state`. Each passed 3/3 exact isolated runs. Two fixed normal-parallel crate-only runs were mixed: 85/0/1 and 82/3/1, reproducing the same three failures without workspace execution. Temporary diagnostics exposed `repo.file-info` total observer-budget exhaustion before later Git observation children launched; the duplicate-call execution counts remained correct. One test-only Git lookup cache experiment still failed 82/3/1 and was removed.

Task 444 outcome is **A — COMPOSED OBSERVER TIMEOUT / BUDGET EXHAUSTION PROVEN**, with no correction retained. Its one diagnostic workspace contrast exited 101 in the Desktop test binary after 299 passed, 22 failed, 18 ignored under heavy instrumentation; runtime-codex was not reached. This diagnostic does not revise Task 443's canonical Desktop 3/3 PASS. See `docs/plans/2026-09-27-task-444-composed-observer-bridge-suite-stability.md` and its preserved logs. A production observer-budget or validation change would require a separate authorized production-defect task. Task 438 remains paused; Task 441 remains **STOPPED / NOT CERTIFIED**. No commit or push was made.

## Task 448 re-entry and integrated closure

The earlier STOP and diagnostic outcomes above remain historical. Task 445's bounded `repo.file-info` budget correction passed the Task 448 canonical `rah-tools` gate three times on the isolated worktree (348 passed, 0 failed each), plus package check/Clippy, the three exact Task 444 bridge regressions, and two ordinary-parallel `rah-runtime-codex --lib` runs (85 passed, 0 failed, 1 ignored each). The historical Task 445 repository-commit three-failure event remains an unresolved transient / suite-context failure, without an identified mechanism. The binary patch was checked and applied cleanly to this integration tree.

The integrated gates passed in order:

- `rah-runtime-codex --lib` twice: 85 passed, 0 failed, 1 ignored each.
- Task 443 canonical Desktop harness three times: 321 passed, 0 failed, 18 ignored each; normal completion. Evidence: `F:\Temp\rah448-integrated-desktop\` (three dated run directories).
- Task 448 canonical `rah-tools` harness twice: 348 library tests passed, 0 failed each. Evidence: `F:\Temp\rah448-integrated-rah-tools-1` and `F:\Temp\rah448-integrated-rah-tools-2`.
- `cargo test --workspace` twice: exit 0 each. Evidence: `F:\Temp\rah448-workspace-1.log` and `F:\Temp\rah448-workspace-2.log`.
- `cargo fmt --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, the Tauri permission inventory test, frontend JavaScript syntax check, frontend authority test, and `git diff --check`: PASS. `cargo metadata --no-deps --format-version 1`: 13 packages, 13 workspace members, version 0.32.0, edition 2024.

**A — INTEGRATED WINDOWS TEST STABILITY RESTORED.** Task 439 and Task 440 are closed; Task 443's canonical Desktop gate and Task 448's canonical `rah-tools` gate are operational; Task 444's file-info budget root cause is diagnosed; Task 445's correction is certified in the integrated tree. Task 446 and Task 447 remain historical diagnostics, and their repository-commit failure mechanism remains unresolved. No repository-commit ceiling or authorization semantic changed. HostExplicit remains exactly 11; v0.33 product capability remains NONE SELECTED. No ADR, dependency, authority, or Codex runtime policy change. Task 438 stays paused until natural push CI succeeds on the exact final HEAD.