# Task 515A — staged-diff diagnosis and Desktop acceptance

Starting HEAD: `0818efbf999f3592bf7195fcd05578e2bb69526a`.

Preserve Task 515 frontend WIP and historical 514/514C reports. Initial status,
diff, copied files, SHA-256 manifest and original complete failing logs are retained
under ignored `target/task515a`. No staging or publication before all required gates.

1. Reproduce the qualified snapshot matrix test alone with complete output and retained child exit evidence.
2. Trace staged-diff Tool errors with bounded private diagnostics, keeping FrontendError unchanged. Compare baseline only if isolated failure persists.
3. Correct only a demonstrated defect or establish a reliable corrective infrastructure condition; retain failed evidence.
4. Complete required deterministic gates and normal Windows production build.
5. Verify actual redesigned Desktop layout, restart persistence, native chat/Tool/Cancel/recovery and authority controls, with screenshots.
6. Publish only after all gates and actual acceptance pass; verify exact-head natural CI. No release preparation.

Original failure: workspace invocation `cargo test --workspace`, Desktop 338 passed,
1 failed, 13 ignored in 320.76s; `StagedDiffExecution` at main_tests.rs:2295 during
the final fresh Untracked B snapshot. Status and worktree diff precede staged diff;
the original log does not retain the underlying Tool error.

## Preserved starting inventory

Tracked WIP: `crates/rah-desktop/frontend/index.html`, `status.js`, `styles.css`.
Untracked Task 515 WIP: `crates/rah-desktop/frontend/layout.js`,
`workspace_layout_test.js`, `docs/plans/2026-10-09-task-515-desktop-workspace.md`.
Historical untracked reports: Task 514 and Task 514C. Nothing was staged.
Initial tracked diff: three files, 52 insertions, three deletions.

`target/task515a/starting-manifest.json` records all eight original dirty/untracked
paths, lengths and SHA-256 values; `backup/` contains their exact bytes.
Tracked binary patch SHA-256:
`943A114D2189A8112A899CA838AA5CE13F8F77B457AC3012AFF35204A2B4ACAF`.
Original full failing stdout SHA-256:
`3A18C98FEE99856CAEEFE4CAE75C8D00424FDB28A58F1ED5CA9FC6E277271949`.
Both original stdout and stderr were copied without overwriting Task 515 evidence.

## Reproduction and underlying-error investigation

Qualified focused command:

```powershell
cargo test -p rah-desktop tests::repository_snapshot_matrix_isolated_repositories_and_replacements -- --exact --nocapture
```

Exactly one test executed: **1 passed, 0 failed, 351 filtered**, 42.50 seconds;
retained Cargo process exit **0**. Complete output and process identity are in
`focused-original.stdout.log`, `.stderr.log`, `.process.json` under the evidence
directory. The runner self-tests retained child exit 0 and exit 7 correctly.

Git discovery uses the first `where.exe git.exe` result, canonicalized by the
fixture helper. Current discovery yielded `C:\Program Files\Git\cmd\git.exe`,
version `2.55.0.windows.5`, SHA-256
`78211C7ED73988DA93A6D8A33D47EC6187F464D7EA2A9A00C182BBD7A1ECF30F`.
The historical failing run did not record an executable hash, so identical
historical Git bytes cannot be certified retrospectively.

Source trace: `desktop_repository_snapshot_with_review` executes status, worktree
diff, then `repository.staged_diff.execute`; `RepositoryDiffStagedTool` delegates
to `execute_fixed_diff`, which takes the repository lease and runs pre-HEAD,
raw/numstat/patch and post-HEAD with repeated repository revalidation. The original
error mapping proves status and worktree diff executions returned successfully
before staged diff failed. It does not identify a failed child or its exit code.

Each matrix fixture has an independently created timestamp/atomic-sequence root.
Clean A, Untracked B, Staged C and Modified D owners remain in scope through the
final fresh B snapshot; their `Drop` removes each root at test exit. This establishes
intended lifetime, not the validity of the original fixture at the failure instant.
The Task 515 report's selected-C description was inaccurate: source line 2295 is
the final fresh Untracked B snapshot. The historical report itself is preserved.

To compare original suite conditions, one diagnostic `cargo test --workspace` ran
with default test parallelism (`RUST_TEST_THREADS` unset), default `target`, and
`RAH_TEST_TARGET_DIR` set to that target as in Task 515's runner. Temporary,
opt-in diagnostics retained bounded Tool error text and root/.git/index existence
on staged failure; the existing live-test-support path also gained failed diff
child exit/timeout/overflow/output-length diagnostics. No raw repository contents
or credentials were printed. FrontendError and execution policy were unchanged.

This diagnostic full-suite run **passed**, retained Cargo exit **0**:

| Observation | Result |
| --- | --- |
| Desktop, including exact matrix test | 339 passed, 0 failed, 13 ignored; 269.20s |
| Entire workspace, including integration/doc tests | 1078 passed, 0 failed, 18 ignored |
| Native runtime adapter tests, within workspace | 23 passed |
| Neutral runtime tests, within workspace | 18 passed |
| Exact HostExplicit allowlist and eleven-kind presentation tests | PASS |

Complete stdout SHA-256:
`A7C5B848AA38F5A87419065297FE21C1A2598DA6467939C84DFDFB91E2A55B4D`.
The diagnostic run made no staged-error record because the target passed.
Default parallel execution was not reduced to obtain this result. Concurrent Git
processes were observed, but that is not resource exhaustion or causal evidence.
No timeout extension, retry loop, assertion change, ignored test, mocked success,
authority change or process killing was used.

No clean baseline execution was needed under the requested isolated-failure
condition: the isolated failure did not persist. Neither successful invocation
establishes baseline-only, WIP-only or concurrency-only causation. The target was
**not reproduced in these two fresh invocations**. The original failure remains
real retained evidence with its underlying cause unknown.

Temporary diagnostics were saved as `temporary-diagnostics.patch` and then removed
by restoring the two backed-up Rust files. Both match their original SHA-256.
All eight original dirty/untracked files remain byte-identical, verified in
`preserved-wip-verification.json`. No product/test correction is demonstrated.

## Acceptance and publication gates

The task requires a resolved blocker or reliable corrective condition before
Windows acceptance. Neither is established by these passes. Thus actual Desktop
acceptance was not resumed, and no redesigned Windows screenshot was generated.
Three panels, both splitters, keyboard resizing, collapse/restore, Focus Chat,
Reset Layout, real close/relaunch persistence, corrupt/unsupported preference
fallback and narrow-window controls remain preliminary browser evidence only.
Actual runtime controls, llama.cpp Connect, first/second turns, model Tool
round-trip, Cancel/recovery, disconnect/reconnect, missing-key recovery, Effective
Authority and Repository/Trusted Profile accessibility remain unverified for the
redesigned production Desktop. No paid/OpenAI live call was made.

Task 515's frontend/browser and Tauri inventory results are retained without
claiming a fresh rerun. A separate canonical Desktop gate, fresh frontend/browser
gates, fmt, workspace check, strict all-feature Clippy and normal production release
build were not run in this diagnostic task. `git diff --check` passed after removal
of diagnostics. The diagnostic workspace pass is not complete release-quality or
actual UI acceptance evidence.

HostExplicit remains 11; its exact allowlist tests passed in the diagnostic suite.
There is no final backend, dependency, ADR, permission, HostExplicit, repository
authority, neutral runtime/provider, credential or layout persistence schema change.
Only this new Task 515A report is added to the original preserved WIP at closure.

## Final disposition

**D — STAGED-DIFF ROOT CAUSE UNRESOLVED.** No natural failure reproduced and no
underlying Git/Tool failure was captured. There is insufficient evidence to name
an external infrastructure cause or implement a safe correction. The current
successful diagnostic suite does not explain or repair the historical failure.

Starting and final HEAD: `0818efbf999f3592bf7195fcd05578e2bb69526a`.
No commit, staging, push, new CI, tag, version change or v0.34.0 preparation.
Task 515 remains **incomplete**. Further work requires natural failure evidence or
an explicit decision on acceptance of the unresolved historical validation risk;
no additional broad repetition was performed merely to accumulate green results.
