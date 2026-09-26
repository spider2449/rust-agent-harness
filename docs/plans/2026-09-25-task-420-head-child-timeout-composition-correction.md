# Task 420 — HEAD Child Timeout Composition Correction

Date: 2026-09-25

## Starting checkpoint and scope

- Starting HEAD: `8c5f47158cb61a02d7285dda208e3e4d52fb9526` (`docs: localize staged observation timeout semantics`).
- Index empty; starting `git diff --check` passed, with existing LF/CRLF warnings.
- Frozen Task 412 eight-file binary diff fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- Starting Task 417 three-Rust-file binary diff fingerprint: `0525250ea4936b0baa37451bc7962bfbecf17af6`.
- Task 419 reproduced two post-HEAD rejections before child spawn: at 5.300s elapsed with 9.699s aggregate remaining, and 6.235s elapsed with 8.764s aggregate remaining. Both received zero HEAD allowance from the old formula.

## Correction and invariants

`RepositoryObserver::run_with_budget` in `crates/rah-tools/src/repository_observer.rs` previously calculated `command_remaining = command_ceiling - staged_elapsed`, then `min(command_remaining, aggregate_remaining)`. For staged HEAD this meant `min(5s - staged_elapsed, 15s - staged_elapsed)`; at 5s elapsed it rejected HEAD while the aggregate retained 10s.

The private `child_timeout(ceiling, started, aggregate, now)` helper now uses `min(command_ceiling, aggregate_remaining)` when an aggregate is present. Without an aggregate, it preserves the existing command-observation elapsed accounting. The caller invokes it after repeated layout validation and before building or spawning the observer child. Zero allowance returns the existing `repository observation exceeded its total timeout` error. No partial diff, retry, stale result, or selector update is introduced.

`DIFF_TIMEOUT` remains 15s, `FILE_INFO_TIMEOUT` remains 5s, and layout `PROBE_TIMEOUT` remains 5s. `execute_fixed_diff_while_leased` still creates its one `Instant::now()` immediately before pre-HEAD, passes it through pre-HEAD, raw, numstat, patch, and post-HEAD, and never refreshes it. Task 417's layout probe clamp remains `min(PROBE_TIMEOUT, aggregate_remaining)` and fails closed at zero. The eight layout probes retain their count, order, arguments and interpretation; validation still precedes every observer command. The five observer commands retain their count, order and Git arguments. Lease, repository identity/currentness, authority, selectors, and external error mapping are unchanged.

The staged HEAD allowance for elapsed 0, 2, 4, 4.9, 5, 6 and 10 seconds is 5s. At 12s elapsed it is 3s; at 14.9s it is 0.1s; at 15s the helper rejects before child spawn. Raw, numstat and patch retain effective allowance equal to aggregate remaining because their 15s command ceiling equals the aggregate limit.

## Deterministic regression tests

Three private tests in `repository_observer.rs` use fixed `Instant` arithmetic and no sleeps. `staged_head_child_timeout_composes_ceiling_with_aggregate_remainder` checks the full required boundary table, including 6s elapsed with 9s aggregate remaining and a 5s HEAD allowance. `staged_child_timeout_uses_aggregate_remainder_for_diff_commands` checks the 15s command ceiling with 9s aggregate remaining. `exhausted_staged_budget_rejects_child_before_spawn` checks the existing timeout error from the pre-spawn helper at 15s elapsed. All three individual focused runs passed.

## Static diff audit

The production correction is confined to the timeout calculation in `repository_observer.rs` plus its private helper; the same file contains the three tests. The pre-existing Task 417 changes in `repository_diff.rs` and `repository_git_layout.rs` remain. Comparison with the starting diff found no changes to timeout constants, aggregate timer start, deadline refresh, probe clamp, probe count/order, observer command count/order, Git arguments, repeated validation, lease, identity/currentness, selectors, error behavior, retries or sleeps.

## Validation

The correction and plan file were already present in the dirty worktree at the start of this execution. The starting HEAD was verified as `8c5f47158cb61a02d7285dda208e3e4d52fb9526`; the index was empty. The Task 412 fingerprint was verified at both start and close as `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`. The requested Task 417 starting Rust fingerprint `0525250ea4936b0baa37451bc7962bfbecf17af6` is the supplied expected value, but this execution could not independently reproduce it because the correction and regression tests were already in `repository_observer.rs` at initial inspection. The final combined Task 417 + Task 420 Rust fingerprint is `d3250597309d0ae38cd325b5b13ac060c94169be`.

The primary helper is private `child_timeout` in `repository_observer.rs`, called by `RepositoryObserver::run_with_budget`. Its arguments are `(ceiling: Duration, started: Instant, aggregate: Option<(Instant, Duration)>, now: Instant)`. The current aggregate branch returns `min(ceiling, limit - elapsed)` with saturating subtraction; zero returns the existing total-timeout error before observer child construction/spawn. The no-aggregate branch keeps the existing command-local elapsed allowance. The correction was present in the initial dirty source; no production source was edited during this execution.

Static source review confirmed: `DIFF_TIMEOUT = 15s`, `FILE_INFO_TIMEOUT = 5s`, and `PROBE_TIMEOUT = 5s`; the aggregate timer remains created immediately before pre-HEAD and is passed unchanged through the five observations; no refresh was added; layout probes remain clamped by `min(PROBE_TIMEOUT, aggregate_remaining)`; eight probes and five observer commands retain order and arguments; repeated validation remains before each observer command; lease, identity/currentness, authority, selectors, and error mapping are unchanged; no retry or sleep was added. The helper tests exercise elapsed 0, 2, 4, 4.9, 5, 6, 10, 12, and 14.9 seconds, aggregate exhaustion, and raw/numstat/patch equivalence. These deterministic tests passed in each of three successful `rah-tools --lib` runs (346 passed, 0 failed, 0 ignored per run). Run 3 took 1054.30s. Exact run 1 and run 2 durations were not retained in the command output. The tests use fixed `Instant` arithmetic and no real sleeps.

Focused review matrices passed: opaque review 5/5 (individual wall durations 14.502s, 15.413s, 15.249s, 15.282s, 14.242s); binary review 5/5 (13.499s, 13.294s, 13.142s, 13.234s, 13.887s). The 20-test `repository_commit::tests::` family passed 3/3: 20 passed, 0 failed, 0 ignored each; durations 418.58s, 301.44s, and 299.26s.

Full `rah-tools` package run 1 passed; the prescribed serial loop proceeded to run 2. Run 1's complete aggregate counts and wall duration were not retained in the captured output. Visible package binaries included 346 library tests, fixture binary 0 tests, `cargo_version` 3 passed/1 ignored, conformance 1 passed, execute policy 12 passed, git stage 6 passed, git status 4 passed/1 ignored, git unstage 6 passed, create-file 11 passed, delete-file 11 passed, repository diff 4 passed, staged diff 7 passed, file info 4 passed, repository mutation 10 passed, nested-boundary 1 passed, and rename-preparation 24 passed. No failure was observed in run 1.

Full package run 2 exposed the independent failure `repository_file_info::tests::linked_file_info_reads_the_selected_worktree_head` (`FAILED`). This is outside the timeout-composition correction. Per the task rule, run 2 was interrupted after observing that failure; it was not rerun. Its complete failure detail and test summary were not retained by the truncated command output. This failure stops Task 420 validation here.

Not run after the independent package failure: the second complete `rah-tools` package pass; Desktop A/B/C/D matrices; the three full Desktop package passes; `cargo fmt --check`; `cargo check --workspace`; workspace Clippy; optional workspace tests. `git diff --check` passed at final inspection. No Task 420 PASS verdict is claimed.

Final dirty paths are the eight frozen Task 412 files, the three Task 417/420 Rust files, and this Task 420 plan artifact. Nothing is staged. No commit, push, or tag occurred.

## Audit and release boundary

STOP — VALIDATION STILL HAS INDEPENDENT INTERMITTENT FAILURE. Task 412 remains frozen. Task 420 makes no commit, push, or tag. Do not start Task 421 automatically. After the package failure is investigated under its own authorization, the recommended next scoped task remains Task 421 — Shared Staged Deadline and HEAD Timeout Composition Independent Audit; it must independently audit the combined Task 417 and Task 420 source and evidence before any bounded implementation commit or release-validation resume.
