# Task 443 — Canonical Windows Desktop deterministic gate harness

Disposition: **A — CANONICAL WINDOWS DESKTOP GATE OPERATIONAL — THREE RUNS PASS**. This does **not** certify Task 441 or Task 438. Base committed HEAD: `deba6a10d65ce5db9842a77db3420a89cc0201ec`. All work remains uncommitted in `F:\Temp\rah-task441-integrated-desktop-stability`.

## Prerequisite and invocation contract

Package-only `cargo test -p rah-desktop --bin rah-desktop` does not build the sibling `rah-mcp-echo-server.exe` and `rah-plugin-echo.exe` fixtures. Provider-composition tests resolve these from `RAH_TEST_TARGET_DIR\debug`; absent helpers previously produced false Desktop failures. `scripts/windows-desktop-test-gate.ps1` resolves its repository root from the script location, defaults to the ignored worktree-local `target\windows-desktop-gate`, makes that path absolute, and sets both `CARGO_TARGET_DIR` and `RAH_TEST_TARGET_DIR` to it for the full invocation. It builds both helpers, verifies both files, then runs exactly one unfiltered, ordinary-parallel `cargo test -p rah-desktop --bin rah-desktop -- --nocapture`. `-PrepareOnly` checks fixture preparation without a Desktop run. `-TargetDirectory` and `-OutputDirectory` allow explicit paths, including spaces.

Each invocation writes uniquely named local stdout, stderr, and JSON status files under the supplied output directory or `%TEMP%\rah-windows-desktop-gate`. These logs are outside the repository and are not committed. Status records start/end, elapsed seconds, target, build and test exit codes, and watchdog result. The harness preserves Cargo failure and returns nonzero for `TEST_FAILURE`, `HELPER_BUILD_FAILURE`, `HELPER_MISSING`, `WATCHDOG_TIMEOUT`, and `HARNESS_ERROR`. It never retries, filters, or switches to serial tests. It waits for libtest's final summary even after an individual `FAILED` line.

The Desktop child and helper build each have a 10 minute wall-clock watchdog. This exceeds the observed ordinary Desktop suite duration of roughly 4–5 minutes and catches the previously observed approximately 14 minute stall. It is independent of repository observer timeouts. On expiry, the harness records Windows PID, parent PID, name, and whether each relevant process belongs to its child tree; then `taskkill /T /F` targets only the spawned Cargo PID. No process is killed by name.

## Harness self-validation

- PowerShell parser reported no syntax errors.
- The deliberate `-SelfTestFailure` child exited 23; the harness reported `TEST_FAILURE` and exited 23. Its local evidence directory is `F:\Temp\rah-windows-desktop-gate\20260927-213452-496-b68319d6284741ef8ed973735c9706a5`.
- `-PrepareOnly` built and found both helpers, exited 0, and wrote stdout, stderr, and status: `F:\Temp\rah-windows-desktop-gate\20260927-213540-362-d4ad39ea8f704d27bebdd5050091242b`.
- The initial self-validation exposed two PowerShell wrapper defects (blank redirected `Start-Process` exit status and scalar `.Count` under strict mode). Both were corrected before the three canonical runs. Earlier diagnostic invocations were harness development checks, not Desktop certification runs.

## Fixed canonical Desktop matrix

All runs used the same source tree and `F:\Temp\rah-task441-integrated-desktop-stability\target\windows-desktop-gate`. Each invocation built/verified the helpers and produced independent evidence. Elapsed values include helper build and Cargo preparation; libtest duration is shown separately.

| Run | Result / exit | Passed | Failed | Ignored | Elapsed / libtest | Watchdog | Evidence directory |
| --- | --- | ---: | ---: | ---: | --- | --- | --- |
| 1 | PASS / 0 | 321 | 0 | 18 | 454.150 / 275.04 s | no | `F:\Temp\rah-windows-desktop-gate\20260927-213546-225-8aa91d7284b8477aad05f56cee9d08cd` |
| 2 | PASS / 0 | 321 | 0 | 18 | 292.595 / 277.89 s | no | `F:\Temp\rah-windows-desktop-gate\20260927-214335-619-d66d69da48a04cff81a2ea455dfe2244` |
| 3 | PASS / 0 | 321 | 0 | 18 | 280.402 / 278.81 s | no | `F:\Temp\rah-windows-desktop-gate\20260927-214837-006-3caed19cc6fd4e61b08cf71504f4f2f3` |

The canonical Windows Desktop gate is operational. The older incomplete Task 441 run-2 log remains historical evidence, not a diagnosed root cause. No complete Desktop panic was captured in these three passes.

## Task 441 continuation and stop

After the three passes, the first required ordinary-parallel `cargo test --workspace` used the same target environment and exited 101. Its full local log is `F:\Temp\rah-task443-workspace-1.log`. `rah-runtime-codex --lib` finished **82 passed, 3 failed, 1 ignored**. The three failed tests were:

- `bridge_tests::trusted_profile_composed_observers_advertise_canonical_schemas_and_observe_read_only` — assertion at `bridge_tests.rs:4257`; bridge response `success:false`, text `RAH tool execution failed`.
- `bridge_tests::composed_observer_deduplication_is_call_identity_not_input_memoization` — assertion at `bridge_tests.rs:3633`; observed `Bool(false)` instead of `true`.
- `bridge_tests::composed_repository_observers_verify_multi_patch_post_state` — assertion at `bridge_tests.rs:4257`; bridge response `success:false`, text `RAH tool execution failed`.

The log does not expose the underlying failed operation or prove a cause. The required gate failed, so workspace run 2 and standard deterministic validation were not started. Task 441 remains **STOPPED / NOT CERTIFIED** pending a narrow diagnosis of this new workspace evidence. No production or test-behavior patch, commit, push, tag, or release followed. Task 438 remains paused. HostExplicit remains exactly 11; v0.33 product capability remains NONE SELECTED; RAH version remains 0.32.0. No dependency, ADR, authority, timeout, or Codex policy change was made.

The watchdog timeout path was reviewed and parses, but no real timeout occurred during self-validation or the three passing Desktop runs; its process snapshot and tree termination behavior remains unexercised by this evidence.
