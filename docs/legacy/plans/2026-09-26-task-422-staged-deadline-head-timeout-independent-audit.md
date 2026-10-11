# Task 422 — Shared Staged Deadline and HEAD Timeout Composition Independent Audit

Date: 2026-09-26

Status: **PASS — SHARED STAGED DEADLINE AND HEAD TIMEOUT CORRECTION INDEPENDENTLY VERIFIED**

## Starting checkpoint and dirty scope

- Starting committed HEAD: `8c5f47158cb61a02d7285dda208e3e4d52fb9526` (`docs: localize staged observation timeout semantics`).
- Index was empty. `git diff --check` passed at entry.
- Starting dirty scope:
  - Frozen Task 412 release-preparation files: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/RAH_V0.32_RELEASE_GATE.md`, `docs/plans/2026-09-25-task-412-v0.32-release-preparation.md`.
  - Correction: `crates/rah-tools/src/repository_diff.rs`, `crates/rah-tools/src/repository_observer.rs`, `crates/rah-tools/src/repository_git_layout.rs`.
  - Evidence: `docs/plans/2026-09-25-task-420-head-child-timeout-composition-correction.md`, `docs/plans/2026-09-25-task-421-linked-file-info-intermittent-failure-disposition.md`.
- Starting Task 412 eight-file binary-diff Git object fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- Starting corrected three-Rust-file binary-diff Git object fingerprint: `d3250597309d0ae38cd325b5b13ac060c94169be`.
- Both fingerprints matched their required frozen values. Nothing was staged.

## Independent policy reconstruction

### Task 416 aggregate policy

Task 416's original design/history evidence establishes one `DIFF_TIMEOUT = 15 seconds` budget for one staged `IndexVsHead` observation. Its timer begins once, immediately before pre-HEAD, and covers pre-HEAD, raw staged diff, numstat staged diff, patch staged diff, post-HEAD, every repeated layout validation, and supervised Git children. The deadline is not refreshed between phases. Exhaustion rejects the whole observation: no partial diff, retry, stale result, or selector publication.

### Task 417 layout-probe enforcement

The correction forwards the original `(started, DIFF_TIMEOUT)` only through the staged observation path. Each of the eight fixed validation probes computes its effective child timeout as `min(PROBE_TIMEOUT, aggregate_remaining)`. `PROBE_TIMEOUT` remains five seconds. Zero remaining is rejected before a supervised Git probe is spawned; if an active probe times out at aggregate exhaustion, the existing aggregate-timeout error is returned. With no aggregate, `probe_timeout` returns the existing five-second `PROBE_TIMEOUT`, preserving non-staged behavior.

The probes remain in this order: top-level, private Git dir, common Git dir, bare status, superproject, index path, HEAD path, worktree registration. Their eight argument vectors and result interpretations match the committed baseline. Validation still executes before every observer command. A complete successful staged path therefore performs 8 × 5 = 40 layout probes, subject to fail-closed early termination.

### Task 419 defect reconstruction

The prior staged HEAD calculation combined a five-second command ceiling with staged elapsed time, effectively `min(FILE_INFO_TIMEOUT - staged_elapsed, DIFF_TIMEOUT - staged_elapsed)`. The committed Task 419 record documents direct post-HEAD pre-spawn reproductions at 5.300 seconds elapsed with 9.699 seconds aggregate remaining and at 6.235 seconds elapsed with 8.764 seconds remaining. The former formula gave HEAD zero allowance even while the aggregate deadline had substantial time remaining. The defect was the staged HEAD command allowance composition, not a nearby layout timeout.

### Task 420 correction reconstruction

The current helper computes `aggregate_remaining = aggregate_limit - aggregate_elapsed`, then applies `min(command_specific_ceiling, aggregate_remaining)`. Staged HEAD gets `min(FILE_INFO_TIMEOUT, aggregate_remaining)`. Raw, numstat, and patch retain their existing `DIFF_TIMEOUT` command ceiling, so their effective timeout is the aggregate remainder. Zero is rejected before constructing/spawning the observer child. The helper's no-aggregate path preserves the existing command-local elapsed allowance.

## Production diff and invariants

- Production changes are classified as **DEADLINE PLUMBING** (`run_diff`, optional aggregate passed to identity/layout validation), **TIMEOUT-COMPOSITION CORRECTION** (`child_timeout` and probe timeout clamp), and **DETERMINISTIC TEST SUPPORT** (pure helper tests). **UNRELATED: none.**
- Visibility changes: `run_diff` and `validate_git_with_budget` are `pub(crate)`, within the existing crate boundary. New budget and timeout helpers are private. No `pub` API, externally reachable schema/API, or authority-facing interface was added.
- Constants, before and after: `DIFF_TIMEOUT = 15s`, `FILE_INFO_TIMEOUT = 5s`, `PROBE_TIMEOUT = 5s`. **CONSTANT DELTA: NONE.**
- `execute_fixed_diff_while_leased` still creates one `Instant` immediately before pre-HEAD and forwards that same instant through the staged five-phase sequence. No restart, reset, or per-phase refresh was added.
- Eight probes remain, in the same order, with unchanged arguments and interpretation. Probe-count delta: none; probe Git-argument delta: none.
- Five observer roles remain in the same order: pre-HEAD, raw, numstat, patch, post-HEAD. The exact baseline command argument vectors are unchanged: the production diff changes only dispatch from `run` to `run_diff`; fixed command policy construction and arguments are untouched. **Command count/order delta: none; Git argument delta: none.**
- Layout validation remains before each of the five observer commands.
- Lease acquisition, lifetime, per-root registry, and cooperating RAH-operation serialization are untouched. **LEASE DELTA: NONE.**
- Pre/post HEAD semantics, repository identity, selected root, generation/currentness, and replacement behavior are untouched. **IDENTITY/CURRENTNESS DELTA: NONE.**
- Stage/Unstage selector derivation, stale selector rejection, commit authorization, `RepositoryObservation` classification, `ToolRegistry` ownership, HostExplicit classification/count, permissions, and IPC/schema are outside the diff and unchanged. **SELECTOR DELTA: NONE; AUTHORITY DELTA: NONE; IPC/SCHEMA DELTA: NONE.**
- Exhaustion continues to fail closed with no partial result, retry, stale cached result, or selector publication. Higher-level direct snapshot `StagedDiffExecution` mapping and refresh `ReviewUnavailable` fallback are unchanged; the diff does not touch their callers or mapping code.
- Diff search found no added retry or sleep; no `retry`, `sleep`, `thread::sleep`, or `tokio::time::sleep` control flow was introduced.

## File-info independence and Task 421 evidence

`RepositoryFileInfoTool::execute` calls `RepositoryObserver::run` for Index, Head, HeadTree, and FileInfoStatus. `run` calls `run_with_budget(..., None)`, which sends `None` through layout validation. It does not call `run_diff`; therefore the staged aggregate is inactive for `repo.file-info`. The no-aggregate branches of `probe_timeout` and `child_timeout` retain the existing five-second probe ceiling and five-second elapsed file-info allowance. The Task 417/420 behavior is structurally inactive on this path.

Task 421's artifact and retained output logs were checked. Its matrix is internally consistent:

- Exact linked file-info test: 5/5 runs passed, one test per run.
- `repository_file_info::tests` family: 3/3 runs passed, four tests per run.
- `rah-tools --lib`: 3/3 runs passed, 346 passed each.
- Full `rah-tools`: 3/3 runs passed, 454 passed, 0 failed, 1 ignored each.
- The package logs contain successful run metadata and zero exit status. The ignored integration test requires an explicitly configured `RAH_GIT_STATUS_EXECUTABLE`.

The original Task 420 failure occurred but was not reproduced; its cause remains unidentified. No missing panic detail or original package count is inferred.

## Independent validation

- Deterministic timeout subset: `cargo test -p rah-tools --lib timeout -- --test-threads=1` — **5 passed, 0 failed**, covering all three probe timeout tests and both observer timeout tests. The observer HEAD table includes 6 seconds elapsed / 9 seconds aggregate remaining → 5 seconds allowance; it also tests full remainder, near exhaustion, and pre-spawn aggregate exhaustion.
- `repository_commit::tests::authorizing_an_opaque_review_never_invokes_commit` — **1 passed**.
- `repository_commit::tests::binary_staged_content_never_produces_or_arms_a_review` — **1 passed**.
- `repository_file_info::tests::linked_file_info_reads_the_selected_worktree_file_and_head` — **1 passed**.
- `cargo test -p rah-tools --lib -- --test-threads=1` — **PASS**, 346 passed, 0 failed, 0 ignored.
- `cargo test -p rah-tools -- --test-threads=1` — **PASS**, all package binaries completed; aggregate 454 passed, 0 failed, 1 ignored, 0 doc tests.
- `cargo fmt --check` — **PASS**.
- `cargo check --workspace` — **PASS**.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **PASS**.
- `git diff --check` — **PASS**.

No intermittent failure occurred during Task 422 independent validation; no rerun was needed.

## Fingerprints and verdict

- Ending Task 412 eight-file binary-diff fingerprint: `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`, unchanged.
- Ending corrected three-Rust-file binary-diff fingerprint: `d3250597309d0ae38cd325b5b13ac060c94169be`, unchanged from Task 421.
- Task 412 files were not edited by Task 422.

```text
PASS — SHARED STAGED DEADLINE AND HEAD TIMEOUT CORRECTION INDEPENDENTLY VERIFIED
```

Commit decision: PASS authorizes a bounded local commit of the three Rust correction files, the Task 420 and Task 421 records, and this Task 422 audit record. No Task 412 release-preparation file is included.

Release-validation resume decision: Task 412 remains **BLOCKED BUT RESUMABLE**. Release preparation is not resumed here. No push or tag is authorized or performed. Do not start Task 423 automatically.

Recommended next task: **Task 423 — Resume RAH v0.32.0 Release Preparation Validation**.