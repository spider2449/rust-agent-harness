# Task 415 — Staged-Diff Observation Timeout Correction

Status: **STOP — STAGED-DIFF TIMEOUT POLICY REQUIRES DESIGN CLARIFICATION**

## Starting checkpoint

- HEAD: `db0d99c2bdda3f5264ad71f960105da5de40cf6d` (`docs: isolate intermittent desktop git observation failures`).
- The eight pre-existing Task 412 paths were dirty: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/RAH_V0.32_RELEASE_GATE.md`, and `docs/plans/2026-09-25-task-412-v0.32-release-preparation.md`.
- Their binary diff fingerprint was `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`.
- No Task 412 files were edited.

## Task 414 evidence and timeout ownership

Task 414's reproduced failure and source map are recorded in `docs/plans/2026-09-25-task-414-intermittent-desktop-git-observation-failure-isolation.md`. The relevant production path is:

```text
RepositoryDiffStagedTool::execute
  -> execute_fixed_diff
  -> execute_fixed_diff_while_leased
  -> pre-HEAD, raw, numstat, patch, post-HEAD
```

In `crates/rah-tools/src/repository_diff.rs`, `execute_fixed_diff_while_leased` revalidates the observer, creates `started = Instant::now()`, and passes that same instant to both HEAD probes and all three diff observations. The timeout constant is `DIFF_TIMEOUT = Duration::from_secs(15)` in `crates/rah-tools/src/repository_observer.rs`.

Before each fixed observer command, `RepositoryObserver::run` revalidates repository identity, calls `repository.validate_git`, validates the observation boundary for relevant command types, then subtracts `started.elapsed()` from the command class timeout. `RepositoryGitLayout::validate_git` performs eight fixed Git probes. Thus all five staged observer commands and their repeated layout probes share the same 15-second elapsed-time budget. The budget check happens after the layout probes and before the next fixed command is spawned. This matches Task 414's observed timeout boundary.

The timer is created at `repository_diff.rs` in `execute_fixed_diff_while_leased`, immediately before pre-HEAD observation, not after a successful independent layout-validation phase. Layout validation is repeated inside every `run`; source does not expose one separate validation phase after which the staged-diff sequence begins. The source documents `run` as using a common timeout and reports an exceeded total timeout.

## Stop reason

The proposed Task 415 correction requires treating layout validation as an independent preflight outside the staged-diff aggregate budget. Current source instead places repeated semantic layout validation inside the shared total observation budget. Starting a fresh 15-second timer after moving or adding a preflight would change that policy; it is not a placement-only correction supported by the current phase structure.

No production or test source was changed. No timeout, Git command, repository validation, identity/currentness behavior, selector behavior, error mapping, retry, or sleep was changed. The source therefore has no “after correction” state. The exact current staged command vectors remain those recorded by Task 414:

```text
pre-HEAD: --no-pager rev-parse --verify -q HEAD
raw:      --no-pager diff --cached --raw -z --no-abbrev --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
numstat:  --no-pager diff --cached --numstat -z --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
patch:    --no-pager diff --cached --patch --no-color --no-prefix --full-index --no-renames --no-relative --no-ext-diff --no-textconv --diff-algorithm=myers --no-indent-heuristic --inter-hunk-context=0 --unified=3 --ignore-submodules=all --submodule=short
post-HEAD: --no-pager rev-parse --verify -q HEAD
```

No correction was made, so no command delta exists. The eight Git-layout probe arguments also remain unchanged. Repository-boundary validation, pre/post HEAD checks, repository identity, selector/currentness, and existing error behavior are unchanged. Retries added: none. Sleeps added: none.

## Validation and remaining work

Per the task's stop conditions, no timing regression test, A/B/C/D stress matrix, Desktop package stress run, formatting check, clippy check, or diff check was started. There is no narrow timing seam assessment beyond the source inspection needed to reach this stop verdict. The required deterministic and stress gates remain unclaimed.

The Task 412 fingerprint was verified both before investigation and at closure as `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3`; the release-preparation diff is unchanged. `git diff --check` returned success for the tracked diff. Task 412 release validation remains untouched and blocked. No commit, push, or tag occurred. Task 416 was not started.

Before implementing a correction, the staged-diff timeout policy needs an explicit decision about whether the existing 15-second total budget includes repeated Git-layout validation. No arbitrary timeout increase or unbounded/retry behavior is proposed here.
