# Task 447 — Repository commit parallel timing localization

Status: **F — DIAGNOSTIC RUN PASSED; ORIGINAL FAILURE STILL UNLOCALIZED**. Task 445 remains **STOPPED / NOT CERTIFIED**. Task 438 remains paused. Base commit: `deba6a10d65ce5db9842a77db3420a89cc0201ec` in `F:\Temp\rah-task445-file-info-budget`.

## Starting point and execution

Task 446 found all three original failing repository commit tests passing in three isolated runs each on both the Task 445 worktree and the clean base. The clean-base normal-parallel crate run passed. Its Task 445 diagnostic failed at instrumentation compilation, so it provided no runtime evidence. The original 345-pass/3-fail output remains unavailable.

Temporary `#[cfg(test)]` timing was gated by `RAH_TEST_COMMIT_TIMING=1`. The first temporary compile attempt failed on a diagnostic-only closing brace. After correction, `cargo fmt --check` and `cargo test -p rah-tools --no-run` both passed before any Task 447 test execution. One logging sanity run used `repository_commit::tests::opaque_review_is_refused_by_another_policy -- --exact --nocapture`; it passed, and records were parseable with no repository contents, branch names, object IDs, paths, messages, or credentials. Sanity log: `F:\Temp\rah-task447-sanity.log`.

The single authorized normal-parallel run was:

```powershell
$env:RAH_TEST_COMMIT_TIMING = '1'
cargo test -p rah-tools -- --nocapture *> F:\Temp\rah-task447-rah-tools-timing.log
$task447Exit = $LASTEXITCODE
Write-Host "Task447 exit code: $task447Exit"
```

Exit code **0**. The library binary reported **348 passed, 0 failed** in 204.27 seconds; every other package test binary passed. All three exact Task 446 target tests passed: `invalid_messages_and_changed_index_refuse_before_spawn`, `opaque_review_is_refused_by_another_policy`, and `policy_generation_head_and_index_races_refuse_before_spawn`. Full log: `F:\Temp\rah-task447-rah-tools-timing.log` (PowerShell UTF-16LE output).

## Measured timing

The following distribution is across the 430 instrumented `RepositoryCommitPolicy::run()` / `run_commit()` calls in the one crate run. Percentiles use the nearest-rank method; units are milliseconds.

| Measure | Count | Median | p95 | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Surrounding semantic Git validation | 430 | 947.107 | 1332.294 | 2103.235 |
| Actual requested Git child | 430 | 106.264 | 203.594 | 408.066 |
| Validation / (validation + child) | 430 | 0.8955 | 0.9334 | 0.9648 |

Every recorded surrounding validation succeeded. Every recorded actual child completed without timeout or overflow. A nonzero exit category is expected for some Git predicates, such as `config --bool` or `diff --quiet`; the diagnostic found no process-execution error. The slowest recorded logical phase was a successful `review` at **25,239.544 ms**. Several phase results marked `error` are expected refusal cases in negative tests; no test failed. The staged review diff was timed as a complete operation, without changes to the observer implementation.

The instrumentation recorded end timestamp and elapsed duration; start is derivable as end minus elapsed. It did **not** separately time the direct `validate_git()` at the beginning of `capture_snapshot()`. Thus the 430 validation distribution covers the per-command surrounding calls, not every validation during commit activity. Also, a `run()` validation failure would emit a validation error record but no complete command record; no such failure occurred. The diagnostic log preserves existing error text locally.

## Classification and decision

**F — DIAGNOSTIC RUN PASSED; ORIGINAL FAILURE STILL UNLOCALIZED.** This run demonstrates substantial successful semantic-validation time relative to requested Git-child time under parallel load. It does not prove validation caused a bounded failure, so class B is unsupported. No timeout, staged observer failure, semantic assertion defect, or other concrete production defect was proven. A production correction task is not justified by this run alone.

Task 445 stays stopped. The next decision must explicitly choose either accepting the original three-failure event as unresolved suite instability and establishing a broader canonical `rah-tools` evidence harness, or a specifically justified stress/reproduction study. Do not repeat this diagnostic run by default.

The temporary Task 447 timing code was removed. `cargo fmt --check` and `git diff --check` passed after restoration; the source diff returned to the pre-Task-447 Task 445 state. No commit, push, Task 445 patch export, Task 441 integration, or Task 438 resumption occurred. No dependency, ADR, authority, or production behavior changed. HostExplicit remains exactly 11; v0.33 product capability remains NONE SELECTED; RAH version remains 0.32.0.
