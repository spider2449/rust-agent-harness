# Task 442 — Desktop Git fixture / observation process-pressure audit

Outcome: **E — RESIDUAL DESKTOP SUITE INSTABILITY REMAINS UNLOCALIZED**.

Base HEAD: `deba6a10d65ce5db9842a77db3420a89cc0201ec`. Diagnosis occurred only in `F:\Temp\rah-task441-integrated-desktop-stability`. Task 438 remains paused; Tasks 439–442 remain uncommitted and uncertified.

## Failed-run evidence boundary

`F:\Temp\rah-task441-desktop-normal-2.log` marks these tests FAILED: `same_size_target_change_rejects_displayed_stage_action`, `review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors`, and `repository_snapshot_matrix_isolated_repositories_and_replacements`. The log is truncated before all three panic summaries and before the final test result. For **each** failure, exact panic line, first failed operation, actual error/result, repository observation stage, `timed_out`, nonzero process exit, and assertion-only status are **not recorded**. They cannot be inferred from the FAILED markers. The same-size test's currentness fingerprint contains canonical path, length, modified time, and SHA-256 digest; no timestamp-granularity defect is established.

## Exact reproductions and hang check

Each failed test ran exactly three times with `--exact --nocapture`. All three were **3/3 PASS**. The nine full outputs are `F:\Temp\rah-task442-exact-<test>-<run>.log`. Every test process exited normally; the prior barrier hang did not reproduce.

## One diagnostic parallel suite

Temporary environment-controlled local instrumentation captured fixture lookup count and duration, fixture Git process duration, repository validation child result, observer command kind, validation time, elapsed and remaining budget before observer launch, child duration, exit, timeout, overflow, and command total duration. It emitted neither repository contents nor raw paths or Git stderr. The instrumentation was removed after the run.

The sole diagnostic command was `cargo test -p rah-desktop --bin rah-desktop -- --nocapture`, with `RAH_TASK442_DIAGNOSTICS=1` and `CARGO_TARGET_DIR=target-task441`. Full local output: `F:\Temp\rah-task442-diagnostic-normal.log`. Result: **318 passed, 3 failed, 18 ignored**, process exit 101, 263.89 seconds. All three Task 441 repository tests passed. The only failures were provider-composition tests requiring `rah-plugin-echo.exe` or `rah-mcp-echo-server.exe` in `target/debug`; that prerequisite was missing because this invocation used `target-task441`. No test hang occurred.

| Measured activity | Count | Duration min / median / max |
| --- | ---: | ---: |
| Native `where.exe git.exe` lookups | 291 | 64 / 135 / 445 ms |
| Fixture Git commands from `git_repository` | 755 | 27 / 270 / 3129 ms |
| Repository validation Git children | 10,921 | 38 / 141 / 1055 ms |
| Observer validation before command | 550 | 377 / 1453 / 2615 ms |
| Observer child commands | 550 | 41 / 134 / 689 ms |

The sampled native lookup concurrency peak was 4. Fixture commands included 148 `init`, 296 `config`, 163 `add`, and 148 `commit` calls. Source call-site counts are not runtime counts. This run shows measurable lookup and fixture process activity, but no failed repository observation linked to it.

| Observer kind | Child count | Remaining budget min / median / max | Child duration min / median / max |
| --- | ---: | ---: | ---: |
| Status | 60 | 7717 / 8586 / 9387 ms | 62 / 142 / 469 ms |
| Worktree diff | 180 | 9048 / 11885 / 14363 ms | 41 / 125 / 557 ms |
| Staged diff | 186 | 7172 / 10238 / 14001 ms | 41 / 143 / 689 ms |

Other observer commands account for the remaining 124 child launches. Across all observer commands, elapsed before child launch was 573/3441/9954 ms and command total duration was 449/1663/2689 ms (min/median/max). All logged observer child exits succeeded, with no timeout, overflow, or execution error. One validation child exited 101 in the intentional invalid-executable classification test. The instrumentation's substantial log volume can affect timing.

## Cache and global-state audit

No native-Git cache experiment was run: the diagnostic suite produced no repository failure to correlate, and repeated lookup activity alone does not establish causation. Static Desktop test search found no normal test mutation of `PATH` or process cwd. An ignored live Git-discovery test temporarily changes `RAH_GIT_EXECUTABLE`, restoring it afterward; other Git-related environment settings found in the relevant tests are child-process scoped. Any future cache decision must still preserve fresh-lookup semantics where needed and avoid cross-test coupling.

No permanent correction, production Git-discovery change, `selected_git_executable()` change, authority change, timeout increase, or test serialization was made. HostExplicit remains exactly 11; v0.33 product capability remains NONE SELECTED; RAH version remains 0.32.0. This audit does not certify Task 441 or authorize a commit, push, tag, or release.
