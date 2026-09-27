# Task 444 — composed observer bridge parallel-suite failure localization

Status: **A — COMPOSED OBSERVER TIMEOUT / BUDGET EXHAUSTION PROVEN**. No corrective patch or certification. Task 438 remains paused. Base HEAD is `deba6a10d65ce5db9842a77db3420a89cc0201ec`. Tasks 439–443 remain uncommitted in this integration tree.

## Initial workspace failure

Source: `F:\Temp\rah-task443-workspace-1.log` (exit 101) and the Task 441 report. The log has the bridge response and panic but no underlying `ToolError`, observer stage, Git exit, or overflow details.

| Exact test | First failed operation | Panic | Visible response |
| --- | --- | --- | --- |
| `bridge_tests::trusted_profile_composed_observers_advertise_canonical_schemas_and_observe_read_only` | First `repo.file-info` call, ID 640 | `bridge_tests.rs:4257` | `success:false`, `RAH tool execution failed` |
| `bridge_tests::composed_observer_deduplication_is_call_identity_not_input_memoization` | New call ID after duplicate response; loop covers `repo.file-info` then `repo.diff`; log does not identify which iteration | `bridge_tests.rs:3633` | `success:false`; response text not printed |
| `bridge_tests::composed_repository_observers_verify_multi_patch_post_state` | `repo.file-info` call ID 734, after successful patch | `bridge_tests.rs:4257` | `success:false`, `RAH tool execution failed` |

## Required reproduction matrix

Each exact test ran three independent times with `RUST_BACKTRACE=1`, `--exact --nocapture --test-threads=1`. All nine exited 0: **3/3 PASS for each test**. Complete logs: `F:\Temp\rah-task444-exact-<test-name>-<run>.log`.

Normal crate-only parallel run 1: **exit 0**, 85 passed, 0 failed, 1 ignored. Log: `F:\Temp\rah-task444-runtime-codex-normal-1.log`.

Normal crate-only parallel run 2: **exit 101**, 82 passed, 3 failed, 1 ignored. The same three composed-observer tests failed with the same visible responses. Log: `F:\Temp\rah-task444-runtime-codex-normal-2.log`. Therefore workspace-level execution is not required to trigger the failure. The two crate runs give a mixed result, not a reliable pass.

## Localization

A temporary diagnostic print at the bridge's `AuthorizedDispatchError::Tool` branch exposed `repo.file-info` errors of `Git repository policy rejected capability: repository observation exceeded its total timeout` for all three failures in an instrumented crate-only run (exit 101). Log: `F:\Temp\rah-task444-runtime-codex-instrumented-1.log`. This identifies **A: repository observer total-budget exhaustion under within-crate parallel load** as the observed failure mechanism. It is not a bridge response-routing or deduplication mismatch.

A second temporary diagnostic at `repository_observer::run_with_budget` showed that the timeout was returned before the `Head` observation child command, after 5,264–6,578 ms had elapsed against the 5,000 ms file-info ceiling. Log: `F:\Temp\rah-task444-runtime-codex-stage-3.log`. Source shows that each observer command performs repository revalidation and Git validation before checking the remaining budget. These measurements localize the budget expiry before the second observation child. They do not separately quantify Git startup time, validation time, OS scheduling, or process pressure; **B remains a plausible contributing mechanism, not a proven independent defect**. There is no evidence here for C, D, E, or F.

The instrumented stage run also had one other failed test, so it is diagnostic only. Its added logging and compilation can affect timing. The temporary source prints were removed; no runtime, observer, fixture, Desktop harness, or timeout change is retained.

## Fixture, lease, and bridge audit

`TestDirectory` uses process ID, nanosecond timestamp, and an atomic sequence. Each `RepositoryObserverFixture` creates its own repository below that directory; its `.git/index` and profile are per fixture. `git()` and `git_output()` use explicit `current_dir(root)` on their child processes. No test helper calls process-global `set_current_dir`, `set_var`, or `remove_var`. `TestDirectory::drop` removes only its own directory, including after ordinary Rust panic unwinding; cleanup errors are ignored. It has no destructor that touches another fixture. `compose()` reads the fixture profile, and `snapshot()` reads only that fixture's Git state and files.

`repository_lease(root)` keys leases by the full repository root (case-folded on Windows), stores weak references, and reuses a live lease only for the same key. Unique fixture roots therefore cannot alias in this registry; stale weak entries create a new lease and cannot block a later test. `FakePeer` uses per-instance channels from `fake_transport()`, with no cross-test response queue. These source audits do not prove every scheduling interleaving, but the observed errors are returned inside the composed Tool before bridge response serialization. The bridge translates `AuthorizedDispatchError::Tool` to the generic failure response.

## Detailed normal-parallel diagnostic

One additional ordinary-parallel crate run with phase diagnostics exited **101**, 82 passed, 3 failed, 1 ignored: `F:\Temp\rah-task444-runtime-codex-diagnostic-final.log`. The three failures again reached `repo.file-info`. The first observer-success failure was `file-info`, before `status`, `diff`, `diff-staged`, or final snapshot. The multi-patch test failed at `file-info` after its patch. Dedupe's duplicate logical call entered the inner Tool once and replayed the same failed result; the new call entered it a second time and failed independently. Counts were 1 then 2, preserving the required call-ID semantics.

In that run, first `repo.file-info` calls reached the index child after about 1.2–2.0 seconds of Git validation. Later validation consumed another 1.2–1.9 seconds per phase. The failing calls reached `head-tree` or `file-info-status` with no remaining part of the 5,000 ms total file-info budget, at 5,130–6,737 ms elapsed. The observer rejected before those children launched. All 30 logged observer children that completed had `timed_out=false` and `overflow=None`; no failed observation child established a nonzero Git exit. This directly proves budget exhaustion, while the exact OS/process source of slow validation remains unseparated.

The same normal crate run performed **575** Windows `where.exe git.exe` lookups: **54,117 / 110,509 / 647,808 microseconds** min/median/max. The cost is substantial in aggregate but is outside each file-info clock and by itself does not establish causality. A single four-thread diagnostic run passed **85/0/1** (`F:\Temp\rah-task444-runtime-codex-four-threads.log`), supporting concurrency sensitivity. One test-only `OnceLock` lookup-cache experiment reduced lookups to one but the normal-parallel suite still failed the same three tests **82/3/1**, with `head-tree` budget exhaustion (`F:\Temp\rah-task444-runtime-codex-cache-experiment.log`). The cache was removed.

## Workspace contrast

The one required diagnostic `cargo test --workspace -- --nocapture` exited **101**; complete output is `F:\Temp\rah-task444-workspace-diagnostic.log`. It reached the Desktop test binary and stopped there after **299 passed, 22 failed, 18 ignored**. Runtime-codex was not reached in this invocation, so it provides no runtime-codex workspace comparison. The heavy observer logging added process and output pressure; the Desktop failures are diagnostic observations, not a replacement for the Task 443 canonical 3/3 PASS. No Desktop harness change was made and no second workspace diagnostic was run.

## Disposition

**Outcome A** is supported by the explicit `ToolError` and before-child budget measurements. B, C, D, and E are not proven. The lookup cache failed its experiment and is not retained. A correction to the observer's production validation or budget behavior would cross Task 444's test-maintenance boundary, so this task stops without such a change. The lower-concurrency result is diagnostic only, not a serial-execution fix. The exact-three, two crate, Desktop, and two workspace regression gates after correction were not run because no correction was retained. Task 441 remains **STOPPED / NOT CERTIFIED**. No commit, push, tag, or release was made.
