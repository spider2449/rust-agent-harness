# Task 445 — Repository file-info budget composition

Status: **LOCALLY CERTIFIED** after Task 448 re-entry. The original `cargo test -p rah-tools` three-failure event remains historical and unresolved. Task 438 remains paused / NOT CERTIFIED; Task 441 remains STOPPED / NOT CERTIFIED until integrated gates pass.

## Evidence and correction

Task 444's normal-parallel `rah-runtime-codex --lib` diagnostics proved that `repo.file-info` exhausted its 5,000 ms logical lifetime before a later observation child launched. The bridge surfaced the underlying Tool failure and call-ID deduplication stayed correct. The original file-info call shared one `Instant` across Index, Head, optional HeadTree, and FileInfoStatus. Each stage revalidated repository identity and Git semantics, but ordinary `run()` passed no aggregate to semantic probes; only the eventual observation child received the five-second remainder. Validation consumed time outside aggregate probe accounting and reduced later observation child time retroactively.

The correction uses a fixed `FILE_INFO_CHILD_TIMEOUT` of five seconds and a `FILE_INFO_TOTAL_TIMEOUT` of four observation stages times that child ceiling: 20 seconds. All four file-info stages pass the same operation start and aggregate to `run_with_budget()`. Each actual child gets the smaller of its unchanged five-second ceiling and the aggregate remainder. Every semantic Git probe also gets the smaller of its existing five-second probe ceiling and that same remainder. Exhaustion fails before the next child or probe spawn. The repository lease, repeated repository identity and Git executable validation, Git layout and worktree semantic checks, reparse defenses, and final revalidation remain in place.

Synthetic-clock tests cover the four named stages at 0, 6, 12, and 18 seconds; child allowances are 5, 5, 5, and 2 seconds. They prove rejection at 20 seconds. A probe test proves five seconds at operation start, two seconds at 18 seconds, and rejection at 20 seconds. The existing linked-worktree file-info regression passed unchanged.

## Adjacent observer audit

| Tool | Classification | Evidence |
| --- | --- | --- |
| `repo.status` | Single-stage | One `Status` child with a fresh start. |
| `repo.diff` | Multi-stage with asymmetric accounting | `WorktreeVsIndex` runs raw, numstat, and patch with a shared start but `run_diff()` passes no probe aggregate. This is the same budget composition pattern; no correction is authorized or made here. |
| `repo.diff-staged` | Multi-stage with explicit aggregate | `IndexVsHead` passes its 15-second aggregate to both Git probes and observation children across two HEAD checks and three diff children. |
| `repo.search` | Multi-stage with explicit outer aggregate | A 15-second `tokio::time::timeout` wraps inventory and subsequent path/text search, including Git validation. It has one Git observation child. |

## Validation

- `cargo fmt --check`: PASS.
- Focused file-info tests: PASS, including the linked-worktree regression.
- `cargo test -p rah-tools`: **FAIL**, 345 passed, 3 failed, 0 ignored in the library tests. The three failures were `repository_commit::tests::invalid_messages_and_changed_index_refuse_before_spawn` (bounded repository HEAD observation), `repository_commit::tests::opaque_review_is_refused_by_another_policy` (observation total timeout), and `repository_commit::tests::policy_generation_head_and_index_races_refuse_before_spawn` (observation total timeout). These are observed failures in the fixed normal-parallel run; their cause has not been established. No retry or concurrency workaround was used.
- `cargo check -p rah-tools` and `cargo clippy -p rah-tools --all-targets --all-features -- -D warnings`: not run after the failed required gate.
- Exact Task 444 bridge tests and two normal-parallel `rah-runtime-codex --lib` runs: not run.
- Integrated runtime, Desktop, workspace, and full validation: not run. The Task 441 integration tree was not modified.

## Authority and Git state

No authority expansion. No Tool schema change. No PermissionLevel change. No HostExplicit change. No model-selected timeout. No dependency or ADR change. The isolated worktree remains uncommitted. No patch export, integration, commit, push, tag, or release occurred.

Task 447's single instrumented normal-parallel rah-tools run passed (348 library tests, 0 failed), but the original three-failure event remains unlocalized; see docs/plans/2026-09-28-task-447-repository-commit-parallel-timing-localization.md. Task 445 remains STOPPED / NOT CERTIFIED.

Task 448 established the canonical Windows `rah-tools` gate and completed three normal-parallel runs, each with 348 library tests passed and 0 failed. The original Task 445 three-failure event remains an unresolved transient / suite-context failure, with no claimed mechanism. See `docs/plans/2026-09-28-task-448-windows-rah-tools-deterministic-gate.md`. Task 445 downstream certification has resumed; its final status depends on those gates.

The resumed isolated gates passed: `cargo check -p rah-tools`; `cargo clippy -p rah-tools --all-targets --all-features -- -D warnings`; all three exact Task 444 composed-observer bridge regressions; and two normal-parallel `cargo test -p rah-runtime-codex --lib` runs (85 passed, 0 failed, 1 ignored each). The isolated correction is **LOCALLY CERTIFIED**. This does not certify the Task 441 integrated tree or authorize a commit.
