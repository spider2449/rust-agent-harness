# Task 419 — Staged Observation Timeout and HEAD Budget Semantics

Date: 2026-09-25

Status: **PASS — HEAD OBSERVER BUDGET SEMANTICS DEFECT ISOLATED**

## Starting checkpoint and frozen scope

- Starting HEAD: `1210e0cbc6bdcc4fd7395896cb1b53f2f9524b65` (`docs: clarify staged diff timeout policy`). Index empty; `git diff --check` passed with only pre-existing line-ending warnings.
- Task 412 eight-path binary-diff fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- Task 417 three-Rust-file binary-diff fingerprint: `0525250ea4936b0baa37451bc7962bfbecf17af6`.
- Task 418 ended **STOP — STAGED-DIFF TIMEOUT VIABILITY REMAINS UNRESOLVED**. Healthy complete observations were about 1.9–2.2 seconds, with validation consuming about 85–90%; that evidence did not justify raising the 15-second aggregate.
- Task 416 remains canonical: one `DIFF_TIMEOUT = 15s` beginning immediately before pre-HEAD, covering five observer phases, repeated validation and supervised Git children, never refreshed, with fail-closed exhaustion. Existing child ceilings remain.

## Historical timeout meaning and chronology

The Task 058 repository-observer research (`docs/RAH_V0.6_REPOSITORY_OBSERVER_RESEARCH.md`) assigns **5 seconds total to `repo.file-info`**, **15 seconds total to `repo.diff` and `repo.diff-staged`**. Task 059, commit `439c388b4fb24dc56e57a4a03593e2cdeee4aa74`, implemented `OBSERVER_TIMEOUT = 5s`: each fixed child policy had a five-second timeout, and `RepositoryObserver::run` subtracted elapsed time since the start of that *file-info* observation. Its task record explicitly calls this a five-second aggregate. Thus the original 5s value was both a child ceiling and the aggregate ceiling of the file-info operation; it was not a separate five-second deadline for staged diff.

Task 060, commit `1afcb80ee133459d1676325350c66badab27ed42`, renamed the constant `FILE_INFO_TIMEOUT = 5s` while adding `STATUS_TIMEOUT = 10s`; `Head` remained in the file-info class. Task 061, commit `e1c0adf1d802f10b1e99963c99280e93b6c7f8b2`, introduced `DIFF_TIMEOUT = 15s` and one shared start across raw, numstat and patch. Task 062, commit `d7167fc30531a752b7b1fae0b4b5fca05278eaf4`, added pre/post HEAD around the three staged commands and passed their common 15-second observation start into `run(Head)`. Its record retained Task 061 bounds; it did not introduce an independent five-second staged HEAD or staged-sequence deadline. This is where the file-info elapsed accounting was unintentionally coupled to staged elapsed. Task 359, commit `02c3eed1c865e38a635ee8ba0f3f6caee1d27a16`, later added eight repeated layout probes before each phase, each with a five-second child ceiling, increasing elapsed time before post-HEAD. Task 417 propagated the staged aggregate into validation probes without changing the pre-existing HEAD calculation.

The `git log -S` searches found `FILE_INFO_TIMEOUT` introduced by Task 060, `file info` by Task 062, and no hit for the literal `rev-parse --verify` expression. The fixed HEAD argv and its use were separately verified in source and Task 059/062 records. No accepted ADR or architecture/security document specifies a staged five-second HEAD deadline.

## Exact current formulas

`execute_fixed_diff_while_leased` creates `started = Instant::now()` immediately before pre-HEAD and passes that *same* instant into pre-HEAD, raw, numstat, patch and post-HEAD. `RepositoryObserver::run_diff` attaches `(started, DIFF_TIMEOUT)` to all staged commands. After validation, `run_with_budget` selects a command timeout and calculates:

```text
command_remaining = command_timeout.checked_sub(started.elapsed())
aggregate_remaining = DIFF_TIMEOUT.saturating_sub(started.elapsed())
effective_allowance = min(command_remaining, aggregate_remaining)
```

`checked_sub` exhaustion, or zero effective allowance, returns `Git repository policy rejected capability: repository observation exceeded its total timeout` before the child spawns. In these formulas `elapsed` is measured near the respective source expression, so nanosecond differences between the two calls are possible.

| Phase | Command ceiling | Current effective formula | Aggregate interaction |
| --- | ---: | --- | --- |
| pre-HEAD | `FILE_INFO_TIMEOUT = 5s` | `min(5s − staged elapsed, 15s − staged elapsed)` | Five-second shared-start branch binds. |
| raw | `DIFF_TIMEOUT = 15s` | `min(15s − staged elapsed, 15s − staged elapsed)` | Aggregate remainder binds. |
| numstat | `DIFF_TIMEOUT = 15s` | same as raw | Aggregate remainder binds. |
| patch | `DIFF_TIMEOUT = 15s` | same as raw | Aggregate remainder binds. |
| post-HEAD | `FILE_INFO_TIMEOUT = 5s` | same as pre-HEAD | Five-second shared-start branch binds after four earlier phases. |

Each of the 40 layout probes uses `min(PROBE_TIMEOUT = 5s, aggregate_remaining)` at its own launch. HEAD is unique among the five observer children because its command ceiling is 5s while the staged aggregate is 15s. Both HEAD calls use exactly the same calculation; post-HEAD is structurally more vulnerable because it follows four validation passes and four prior children. The expected composition for a five-second HEAD *child* ceiling under a 15-second staged total is `min(FILE_INFO_TIMEOUT, aggregate_remaining)`.

| Staged elapsed | Current HEAD allowance | Aggregate remaining |
| ---: | ---: | ---: |
| 0s | 5s | 15s |
| 2s | 3s | 13s |
| 4s | 1s | 11s |
| 4.9s | 0.1s | 10.1s |
| 5s | 0s | 10s |
| 6s | 0s (checked subtraction fails) | 9s |
| 10s | 0s (checked subtraction fails) | 5s |

For elapsed in **[5s, 15s)**, HEAD is ineligible to start despite positive aggregate time. At exactly 5s the command allowance is zero while 10s aggregate remains. The source-level formula proves the boundary without sleeps, process delay injection, or a temporary private test.

## Disposable investigation and failure-only diagnostics

A detached disposable worktree at the starting HEAD received **only** the frozen Task 417 Rust diff. Before diagnostics its three-file `git diff --binary | git hash-object --stdin` was `0525250ea4936b0baa37451bc7962bfbecf17af6`. A dedicated Task 419 Cargo target directory was used; no Task 418 target directory or previously blocked external cache was touched.

Temporary worktree-only diagnostics logged error or timeout paths in `RepositoryObserver::run_with_budget`, `RepositoryGitLayout::probe`, and the pre/post `observe_head` caller. They captured phase, validation probe identity or fixed child command label, elapsed and remaining aggregate time, command/probe ceiling, effective allowance, before/during probe or child, spawn state, timeout/status where returned, and the HEAD-specific 5s limit. No raw host paths were printed. They added no retry, sleep, command, validation, timer reset, ownership change, or result change. A source-level formula proof made a temporary test unnecessary. Diagnostic logging itself can perturb timing slightly; the captured failure occurs after the measured elapsed has already passed 5s.

## Bounded stress results

Commands used the instrumented disposable source and dedicated target. Focused tests used fully qualified names, `--exact --test-threads=1 --nocapture`, five runs each. The family filter was `repository_commit::tests::` (20 tests), with `--test-threads=1 --nocapture`.

| Opaque review run | Result | Command wall seconds | Timeout evidence |
| ---: | --- | ---: | --- |
| 1 | FAIL | 14.675 | post-HEAD rejected before child spawn at 5300ms elapsed, 9699ms aggregate remaining; 5000ms ceiling, 0ms allowance. |
| 2 | FAIL | 17.159 | post-HEAD rejected before child spawn at 6235ms elapsed, 8764ms aggregate remaining; 5000ms ceiling, 0ms allowance. |
| 3 | PASS | 20.277 | none |
| 4 | PASS | 13.789 | none |
| 5 | PASS | 13.665 | none |

| Binary review run | Result | Command wall seconds | Timeout evidence |
| ---: | --- | ---: | --- |
| 1 | PASS | 12.449 | none |
| 2 | PASS | 12.518 | none |
| 3 | PASS | 12.135 | none |
| 4 | PASS | 12.115 | none |
| 5 | PASS | 12.829 | none |

| Commit-review family run | Result | Command wall seconds | Timeout evidence |
| ---: | --- | ---: | --- |
| 1 | PASS, 20/20 | 284.143 | none |
| 2 | PASS, 20/20 | 282.369 | none |
| 3 | PASS, 20/20 | 284.317 | none |

| `rah-tools --lib` run | Result | Command wall seconds | Timeout evidence |
| ---: | --- | ---: | --- |
| 1 | PASS, 343/343 | 869.618 | none |
| 2 | Not run | — | Stop early after sufficient phase capture. |
| 3 | Not run | — | Stop early after sufficient phase capture. |

The two focused failures emitted the same external `Execution` error as Task 417's historical library failures. At the captured failures no HEAD Git child spawned or returned a status. The first log gives 5300ms at the observer check and 5301ms at the post-HEAD error wrapper; the second gives 6235ms and 6236ms, respectively. The aggregate remainder is likewise sampled twice and differs by about 1ms. This is a reproduced mechanism in the same opaque-review test, **not** retroactive phase telemetry for the uninstrumented Task 417 run. The historical binary-review failure phase remains unobserved.

The total-timeout error is shared by true 15s aggregate exhaustion and `FILE_INFO_TIMEOUT`-derived zero HEAD allowance; it does not distinguish them. Task 417's wording alone therefore could not identify its phase. A child that actually times out returns a bounded command completion error instead. No error mapping is changed in this task.

## Decision and next boundary

**PASS — HEAD OBSERVER BUDGET SEMANTICS DEFECT ISOLATED.** The five-second number was historically the file-info total and a fixed child cap. Staged diff has its separate documented 15-second total. Passing the staged start into `Head` also subtracts staged elapsed from the five-second class cap, creating an unintended second shared-start deadline. The captured post-HEAD failures mechanically demonstrate zero HEAD allowance with 8.7–9.7 seconds aggregate remaining. This isolates a real root cause for the reproduced opaque-review timeout; it does not prove every historical intermittent failure used the same phase. The healthy Task 418 measurements still do not justify a larger total.

Task 417's shared-deadline enforcement remains the implementation basis. **No ADR** is needed for composing the existing 5s child ceiling with the existing 15s aggregate. No authority, dependency, Git command, validation, selector/currentness, lease, or error-contract change is proposed. Task 412 remains **BLOCKED** pending correction, independent audit, and repeated `rah-tools`/Desktop/workspace validation.

**Exact recommended Task 420:** `Task 420 — Correct HEAD Child Timeout Composition Under Shared Staged Deadline`. Use `effective HEAD child timeout = min(FILE_INFO_TIMEOUT, aggregate DIFF_TIMEOUT remaining)`; preserve both numeric limits, the timer start, five phases, eight probes per phase, repeated validation, Git commands, lease, authority/currentness/selectors, and fail-closed aggregate behavior. Deterministic no-sleep tests must prove: remaining >5s gives 5s; remaining between 0 and 5s gives the remainder; zero remaining prevents spawn; elapsed >5s with remaining >5s still gives 5s. Task 419 makes no production or test-source correction and does not start Task 420.

## Closure

- Task 417 record: `docs/plans/2026-09-25-task-417-shared-staged-diff-deadline-enforcement.md`.
- Task 418 record: `docs/plans/2026-09-25-task-418-staged-diff-timeout-viability-policy-research.md`.
- Task 419 record: this file.
- Ending Task 412 fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`, unchanged. Ending Task 417 Rust fingerprint: `0525250ea4936b0baa37451bc7962bfbecf17af6`, unchanged.
- The temporary diagnostics were reverted in the disposable worktree; the re-applied frozen Rust diff rehashed to `0525250ea4936b0baa37451bc7962bfbecf17af6`. The disposable worktree and its diagnostic logs were removed. `git worktree list` shows only the main worktree. No Rust or test source was permanently changed.
- Main-worktree `git diff --check`: PASS, with existing LF/CRLF warnings only. The documentation-only staged-set verification and commit are reported in the Task 419 completion report, because a commit cannot contain its own SHA.
- No release validation, push, or tag in Task 419.
