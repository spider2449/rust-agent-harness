# Task 448 — Canonical Windows rah-tools gate and Task 445 re-entry

Status: **A — CANONICAL RAH-TOOLS GATE OPERATIONAL — THREE NORMAL-PARALLEL RUNS PASS**. Base: `deba6a10d65ce5db9842a77db3420a89cc0201ec`.

## Reason

Task 445's ordinary `cargo test -p rah-tools` reported 345 passed and three repository-commit failures, but its complete failure output was not preserved. The exact operation and panic summaries cannot be recovered. Task 446 passed each original test three times in isolation on both the Task 445 worktree and clean base; the clean-base normal crate gate also passed. Task 447's one instrumented normal-parallel run passed 348/0. Across 430 recorded Git command calls, surrounding semantic validation had median 947 ms and p95 1,332 ms; the requested child had median 106 ms and p95 204 ms. There were no observed validation failures, child timeouts, or overflows. The original mechanism remains unlocalized.

## Harness contract and validation

`scripts/windows-rah-tools-test-gate.ps1` resolves the repository from its own location and runs one `cargo test -p rah-tools -- --nocapture` with ordinary libtest parallelism. It lets Cargo finish, preserves combined output, Cargo exit code, HEAD, UTC start/end, duration, and disposition in a distinct evidence directory outside Git. PASS alone returns zero; TEST_FAILURE returns Cargo's nonzero code; HARNESS_ERROR returns nonzero. It sets no test, Rust, or Git environment overrides and has no watchdog, filter, retry, or thread override.

PowerShell parsing passed. Invocation from `$env:TEMP` with a temporary Cargo shim resolved the correct repository HEAD, produced log and JSON metadata, and propagated the shim's exit code 23. The first real invocation exposed a Windows PowerShell stderr handling defect before a Cargo exit code was captured. It is classified HARNESS_ERROR and does not count as a canonical run. The harness was corrected to treat normal native stderr as log output.

## Fixed Task 445 matrix

| Run | Evidence directory | Library summary | Cargo exit | Disposition |
| --- | --- | --- | ---: | --- |
| 1 | `F:\Temp\rah448-canonical-run-1b` | 348 passed, 0 failed | 0 | PASS |
| 2 | `F:\Temp\rah448-canonical-run-2` | 348 passed, 0 failed | 0 | PASS |
| 3 | `F:\Temp\rah448-canonical-run-3` | 348 passed, 0 failed | 0 | PASS |

Both completed runs also passed every package test binary. Each directory contains `cargo-test.log` and `metadata.json`.

## Final disposition

**A — CANONICAL RAH-TOOLS GATE OPERATIONAL — THREE NORMAL-PARALLEL RUNS PASS.** The historical Task 445 three-failure event remains an **UNRESOLVED TRANSIENT / SUITE-CONTEXT FAILURE**; this matrix does not establish its exact mechanism. Task 445 may resume its specified downstream gates. No repository-commit production change, ADR, dependency, authority expansion, commit, push, tag, or release is authorized by this task.

Task 445's resumed isolated gates passed: `cargo check -p rah-tools`, the specified all-targets/all-features Clippy gate, the three exact Task 444 composed-observer bridge regressions, and two normal-parallel `rah-runtime-codex --lib` runs (85 passed, 0 failed, 1 ignored each). Task 445's isolated correction is **LOCALLY CERTIFIED**; integrated Task 441 gates remain separate.

## Integrated Task 441 result

The exported binary patch applied cleanly to the Task 441 tree. Two integrated `rah-runtime-codex --lib` runs passed 85/0/1. Three canonical Desktop runs passed 321/0/18 with normal completion. Two integrated canonical `rah-tools` runs passed 348/0. Two `cargo test --workspace` runs passed. Format, workspace check/Clippy, the three Node checks, diff check, and metadata (13 packages, 13 members, version 0.32.0, edition 2024) also passed. Evidence paths and closure classification are recorded in the Task 441 report. The repository-commit failure mechanism remains unresolved.
