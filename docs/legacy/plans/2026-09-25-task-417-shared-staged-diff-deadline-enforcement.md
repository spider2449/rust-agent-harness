# Task 417 — Shared Staged-Diff Deadline Enforcement

Date: 2026-09-25
Status: **FAIL — SHARED DEADLINE ENFORCEMENT DOES NOT RESOLVE STAGED OBSERVATION INTERMITTENCY**

## Starting checkpoint

- HEAD: `1210e0cbc6bdcc4fd7395896cb1b53f2f9524b65` (`docs: clarify staged diff timeout policy`).
- Dirty Task 412 paths: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/RAH_V0.32_RELEASE_GATE.md`, and `docs/plans/2026-09-25-task-412-v0.32-release-preparation.md`.
- Prescribed eight-path binary-diff fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- No paths staged.

## Frozen policy and baseline topology

Task 416 froze one aggregate 15-second `IndexVsHead` deadline starting at `let started = Instant::now()` in `execute_fixed_diff_while_leased`, immediately before pre-HEAD. It covers pre-HEAD, raw, numstat, patch, post-HEAD, and all repeated validation. Each layout probe retains its five-second ceiling. Exhaustion fails closed without partial result, retry, or selector update.

Baseline signatures: `execute_fixed_diff_while_leased(observer: &RepositoryObserver, baseline: DiffBaseline) -> Result<ToolOutput, ToolError>`; `RepositoryObserver::run(&self, command: ObserverCommand, path: Option<&str>, started: Instant) -> Result<HostProcessOutput, ToolError>`; `RepositoryIdentity::validate_git(&self, git: &Path) -> Result<(), ToolError>`; `RepositoryGitLayout::validate_git(&self, git: &Path) -> Result<(), ToolError>`; private `probe(git: &Path, root: &Path, arguments: &[&str], stdout_limit: usize) -> Result<Vec<u8>, ToolError>`. `HostExecutionPolicy::with_timeout(Duration)` supplies the supervised child ceiling. Before this correction, `run` called `validate_git` before computing `timeout.checked_sub(started.elapsed())`; each probe used `PROBE_TIMEOUT` independently.

## Correction and invariant audit

The original `started` instant is forwarded by `repository_diff::observe_head` and the three diff phases to `RepositoryObserver::run_diff`. That method carries `(started, DIFF_TIMEOUT)` through private observer and layout validation methods to each fixed `probe`. A private pure helper computes `remaining = limit.saturating_sub(now.saturating_duration_since(started))`; zero returns the existing repository observation total-timeout error before a probe policy can execute, and positive time yields `min(PROBE_TIMEOUT, remaining)`. After validation, the fixed command remains bounded by both its existing command allowance and aggregate remaining time. A timed-out layout probe at aggregate exhaustion returns the same aggregate-timeout error.

Changed production files: `repository_diff.rs`, `repository_observer.rs`, `repository_git_layout.rs`. Existing `run` and `validate_git` signatures remain. New `run_diff` and `validate_git_with_budget` methods use the same crate-private visibility as their existing counterpart; `run_with_budget`, `probe_timeout`, and the timing tests are private. The private `probe` signature adds an optional `(Instant, Duration)` budget. No public API or new type is added.

Static diff audit: `DIFF_TIMEOUT` stays 15 seconds; `PROBE_TIMEOUT` stays 5 seconds; the aggregate timer start does not move and no second timer is created. Eight layout probes remain in the same order with identical Git arguments and interpretation. The five staged observer commands and their Git arguments remain unchanged, with validation before each. Lease, root identity/currentness, selectors, fallback/error mapping, retries, and sleeps are unchanged. The only intended behavior change is the timeout allowance for layout Git children and no spawn after aggregate exhaustion. The five-second HEAD child ceiling is retained.

The deterministic seam is the private pure `probe_timeout(aggregate, now)` helper. Three tests cover full budget yielding five seconds, three seconds remaining yielding three seconds, and zero remaining returning the aggregate timeout before `execute_process`. There is no existing fake process runner suitable for a spawn counter or for advancing time across eight probes; adding one would exceed the narrow test seam. No real-time sleeps are used.

## Validation record

- Focused deterministic tests: 3 passed, 0 failed, 0 ignored, 0.00s across the `rah-tools` lib binary; the other 16 test binaries ran zero tests under this filter. The later command-clamp adjustment kept the helper unchanged and the full lib suite executed all three tests again successfully.
- `cargo test -p rah-tools -- --test-threads=1`: **FAILED** in the first `rah-tools` lib binary after 949.30s: 341 passed, 2 failed, 0 ignored. The two failures were `repository_commit::tests::authorizing_an_opaque_review_never_invokes_commit` and `repository_commit::tests::binary_staged_content_never_produces_or_arms_a_review`; both unwrapped `Execution { message: "Git repository policy rejected capability: repository observation exceeded its total timeout" }`. Cargo stopped before running the remaining test binaries. The failures are staged observation aggregate timeouts in commit review fixtures, so the required package gate did not pass. No rerun was used to erase these failures.
- `cargo clippy -p rah-tools --all-targets --all-features -- -D warnings`: PASS.
- Focused Desktop matrix A/B/C/D: not run because the required `rah-tools` suite failed; runs A1–A5, B1–B5, C1–C5, and D1–D5 have no result.
- Three full Desktop serial suites: not run because the required `rah-tools` suite failed. Counts and durations are unavailable. No unrelated Desktop failure was observed or inferred.
- `cargo fmt --check`: PASS. `git diff --check`: PASS. `cargo check --workspace` and workspace Clippy: not run because the Task 417 stress gate was not reached.
- Optional workspace tests: not run; release-grade workspace test belongs after independent audit.
- Ending Task 412 fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`, unchanged.
- Final state: original eight Task 412 paths, three Task 417 production paths, and this Task 417 plan are dirty. Nothing is staged. No commit, push, or tag occurred.

## Verdict and next task

**FAIL — SHARED DEADLINE ENFORCEMENT DOES NOT RESOLVE STAGED OBSERVATION INTERMITTENCY.** The narrow implementation compiles and the deterministic clamp tests pass, but the serial `rah-tools` suite still encountered the aggregate timeout in two staged commit review fixtures. This does not authorize a Task 417 commit or release preparation. The next work needs to investigate completion of the fixed 45-child staged observation within the frozen 15-second policy. **Task 418 — Shared Staged-Diff Deadline Enforcement Independent Audit** remains deferred; it was not started.
