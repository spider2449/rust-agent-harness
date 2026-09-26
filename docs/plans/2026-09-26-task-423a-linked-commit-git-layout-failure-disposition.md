# Task 423A — Linked Commit Git-Layout Failure Disposition

Date: 2026-09-26

## Starting state and frozen release work

- Starting committed HEAD: `83c5c30446a2f566be7b660d45e6d70b98a8d344` (`fix: enforce staged observation timeout composition`).
- `git status --short` tracked dirty paths: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`.
- Starting untracked paths: `docs/RAH_V0.32_RELEASE_GATE.md`, `docs/plans/2026-09-25-task-412-v0.32-release-preparation.md`, `docs/plans/2026-09-26-task-423-v0.32-release-preparation-resume.md`.
- No Rust source or test source was dirty; `git diff -- '*.rs'` was empty; the index was empty. Entry `git diff --check` passed.
- `TASK423_STOP_DIFF_FINGERPRINT=66c7d9b0bfc2e27fb66ae79dc041968a4f3e4b67`, from `git diff --binary | git hash-object --stdin`. It was rechecked immediately before this artifact was created and matched exactly.
- The existing Task 423 release-preparation content was not edited by Task 423A.

## Task 423 evidence retained

Fresh repo.list Windows live recertification: **PASS** at exact source HEAD `83c5c30446a2f566be7b660d45e6d70b98a8d344`. Reported host: Windows 10 IoT Enterprise LTSC 2024, build 26100, x64; rustc 1.98.1; Cargo 1.98.1; Git 2.55.0.windows.5. Task 423 also passed `cargo fmt --check`, `cargo check --workspace`, workspace Clippy, metadata validation, and `git diff --check`.

Two consecutive serial full workspace runs each had 990 passed, 0 failed, 22 ignored. Their summed test-binary durations were 2083.95 and 2094.69 seconds. The original Task 412 intermittent failure pair passed in both. The retained logs at `%TEMP%\rah-task423-workspace-run1.log` and `%TEMP%\rah-task423-workspace-run2.log` each show `repository_commit::tests::linked_commit_uses_only_the_selected_worktree_index_and_branch ... ok` (lines 799 and 791 respectively).

The subsequent standalone `cargo test -p rah-tools -- --test-threads=1` exited 101: 345 passed, 1 failed, 0 ignored in 998.59 seconds. The same linked-commit test failed at `crates\rah-tools\src\repository_commit.rs:1131:74` while unwrapping `review_current_staged_snapshot()`: `Execution { message: "Git repository policy rejected capability: repository Git layout identity validation failed" }`. The complete retained original log is `%TEMP%\rah-task423-rah-tools-release-check.log`. The preceding test was `linked_commit_review_stales_when_its_selected_branch_ref_moves`, which passed. No original rerun or inner probe, exit status, or stderr was retained. Task 423 stopped there. A cause cannot be inferred from the outer error alone.

## Test topology and failure site

The test is `crates/rah-tools/src/repository_commit.rs:1090-1227`; the panic site is line 1131. `WorktreeFixture::new()` is in `crates/rah-tools/src/repository_git_layout.rs:801-856`, with fixture teardown at 859-865. It creates a UUID-qualified temporary base, a main repository on `refs/heads/main`, one initial commit containing `tracked.txt`, then two registered linked worktrees on `refs/heads/linked-a` and `refs/heads/linked-b` by `git worktree add -b`. The main common Git directory is `main/.git`; A and B each have a `.git` gitfile pointing to their separate `main/.git/worktrees/<id>` private directory. Each private directory owns its index and HEAD path. The three worktrees share object/ref storage, not selected worktree authority.

The test edits and stages `tracked.txt` in linked A, captures the main, A, and B layouts, saves the main and A indexes and A HEAD, and composes `RepositoryCommitTool` for **linked A**. The failure happened on the first host review of A's staged snapshot, before any B mutation, authorization, or commit. On the success path, the test then commits an unrelated new file in B and stages another B-only file, authorizes the still-current A review, commits only A, checks A HEAD advanced, B HEAD and index remained unchanged, main index remained unchanged, A index changed, and the B-only file remains staged rather than entering B's HEAD. The expected selected branch is `refs/heads/linked-a`; its old HEAD is the fixture's initial commit. The selected index and HEAD paths are under A's private Git directory.

## Production review and commit call graph

`RepositoryCommitTool::compose` (709-724) creates `RepositoryCommitPolicy::new` (135-159), which captures `RepositoryIdentity` and its `RepositoryGitLayout`, binds the native Git executable, creates an empty hook directory, and obtains the per-root repository lease. `RepositoryCommitControl::review_current_staged_snapshot` (610-614) calls `policy.review` (181-207), which acquires that lease and performs:

1. `capture_snapshot` (329-375): repository/root/layout currentness, `repository.validate_git`, bound executable and hooks checks, ordinary state, selected symbolic branch and HEAD/ref agreement, index admission and staged-entry/boundary validation, `write-tree`, staged delta check, and raw selected index digest. Every `policy.run` (431-439) first calls `repository.validate_git` again before the fixed Git command.
2. `RepositoryObserver::new` for the selected A root, then `execute_fixed_diff_while_leased(IndexVsHead)` (repository_diff.rs:97-135): pre-HEAD, raw staged diff, numstat staged diff, patch staged diff, post-HEAD, with repeated repository/layout validation before each fixed observer command. The staged review carries one 15-second aggregate deadline through those five observer commands.
3. A second `capture_snapshot`, review-semantic equality, binary-content classification, and an opaque review binding. On an all-success review path, there are 21 `validate_git` calls: eight in each snapshot (one initial plus seven fixed `policy.run` calls), and five in the staged diff. Each successful validation runs eight layout probes, for 168 probe invocations. The original outer error does not identify which call failed.

After the first review, `authorize_reviewed_snapshot` (668-677) clears any pending authorization and calls `authorize_review` (221-245) under the lease to recapture and compare the selected snapshot. `RepositoryCommitTool::execute` (741-770) consumes that authorization once and calls `policy.commit` (249-307). The commit path reacquires the lease, revalidates the exact authorized snapshot, validates Git layout again before the one fixed `git commit` spawn (`run_commit`, 416 onward), then verifies the resulting branch/HEAD/commit state or returns an uncertainty/precondition disposition. None of those later steps was reached at the original line-1131 failure.

## Git-layout identity and error propagation

`RepositoryIdentity::capture` (`repository_observer.rs:445-457`) freezes the canonical selected root and filesystem identity plus `RepositoryGitLayout::capture` evidence. For a linked root (`repository_git_layout.rs:209-274`), that evidence includes the `.git` gitfile, private Git directory and its filesystem identity, `commondir`, backlink to the selected gitfile, common Git directory, `worktrees` directory, registration directory, and their identities/content. `RepositoryIdentity::revalidate` (460-469) checks root and layout currentness before `validate_git`; `validate_git_with_budget` (476-484) checks again after Git semantic validation.

`RepositoryGitLayout::validate_git_with_budget` (362-449) runs exactly eight fixed probes in this order: selected top-level; private Git directory; common Git directory; non-bare status; empty superproject; selected index path; selected HEAD path; and `git worktree list --porcelain -z` registration. It compares canonical probe paths and parsed values with the frozen selected layout. Registration must contain exactly one valid selected-root record, neither bare nor prunable. The fixed Git executable and layout filesystem evidence are revalidated around the probes. These checks run on every `validate_git` call described above; the staged observer passes its aggregate deadline, while commit snapshot and commit-specific calls pass `None`.

The outer wording originates in `repository_git_layout.rs:778-779` (`layout_error()`), via `git_support::git_error` (`git_support.rs:48-55`). The same error covers a failed probe result (nonzero or absent exit status, timeout other than a separately recognized exhausted aggregate, output overflow, or excessive stdout), malformed/undecodable probe line, path canonicalization failure, any of the eight value mismatches, or malformed/missing/duplicate/invalid worktree registration. Layout capture/currentness helpers also use `layout_error` for many filesystem and relationship failures. `successful_output` (677-685) discards the Git exit code, timeout/overflow detail, stderr, and probe name; the combined comparison (437-446) discards which equality failed. Process-policy construction/spawn errors can propagate separately, and aggregate exhaustion has its separate total-timeout message. Thus an identity mismatch and a Git process failure **can** produce the same observed outer message, but the original record cannot distinguish them.

## Static timeout-correction impact audit

The committed correction is `83c5c30446a2f566be7b660d45e6d70b98a8d344`, changing `repository_diff.rs`, `repository_observer.rs`, and `repository_git_layout.rs`. It leaves the eight probe commands, order, expected identity, and comparison semantics unchanged.

| Component | Involvement at line-1131 review | Classification |
| --- | --- | --- |
| Task 417 `run_diff` aggregate propagation through the five staged observer commands | Traversed during staged review after the first snapshot | SEMANTICALLY ACTIVE for staged review |
| Layout-probe `probe_timeout(Some(...))` clamp to `min(5s, aggregate remainder)` | Traversed by each staged observer validation | SEMANTICALLY ACTIVE for staged review |
| Task 420 `child_timeout` composition for HEAD and staged diff children | Traversed after each successful staged layout validation | SEMANTICALLY ACTIVE for staged review |
| `capture_snapshot`, its fixed `policy.run` calls, and commit-specific `repository.validate_git` | Use `validate_git(..., None)`; no aggregate | PLUMBING ONLY for correction; five-second probe behavior retained |
| Worktree fixture construction, branch/index mutations, lease registry, layout expected identity, and commit execution | No code changed by correction; commit execution was not reached at original failure | NOT ON PATH for the changed timeout semantics |

The original outer layout error could have come from a non-aggregate snapshot validation or an aggregate-aware staged validation. It is not itself a timeout code. No direct causal link to the correction has been established by static inspection.

## Linked-worktree lifecycle and suite-state audit

`WorktreeFixture::new` runs each native Git fixture command to process completion and asserts successful exit. Its UUID plus process ID and monotonic counter make fixture path reuse within these tests implausible; `Drop` removes only that fixture's own base. It does not run `git worktree prune`. The A/B registration, private directories, indexes, and branch refs are created by Git before policy composition. The immediately preceding linked-review test creates its own fixture and removes only its own base. A search of `crates/rah-tools/src` and `crates/rah-tools/tests` found no `set_current_dir`, `set_var`, `remove_var`, `git worktree prune`, or global/system Git config mutation capable of explaining this case. Production observer children use fixed Git arguments, the selected root as cwd, and an explicit bounded Git environment (`git_support.rs:9-45`). Fixture commands inherit the test process environment but run with an explicit fixture cwd and repository-local config. These findings rule out a *demonstrated* shared-state trigger; they do not prove external interference impossible.

## Disposable exact-source validation matrix

Clean detached worktree: `F:\coding\otherPrj\rust-agent-harness-task423a` at exact HEAD `83c5c30446a2f566be7b660d45e6d70b98a8d344`. Dedicated Cargo target: `F:\coding\otherPrj\rust-agent-harness-task423a-target`. Complete command output was captured per run in `F:\coding\otherPrj\rust-agent-harness-task423a-runs`. No source or environment changes occurred between the five focused runs. Times below are Rust test-binary durations, with command wall times separately noted where useful.

| Matrix | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 |
| --- | --- | --- | --- | --- | --- |
| Exact test, `--exact --test-threads=1 --nocapture` | PASS, 1/0/0, 25.10s | PASS, 1/0/0, 21.71s | PASS, 1/0/0, 20.47s | PASS, 1/0/0, 21.14s | PASS, 1/0/0, 22.14s |
| Existing `repository_commit::tests::linked_commit` family, serial/nocapture (2 tests) | PASS, 2/0/0, 36.15s | PASS, 2/0/0, 33.51s | PASS, 2/0/0, 33.50s | — | — |
| Complete `repository_commit::tests::` family, serial/nocapture (20 tests) | PASS, 20/0/0, 263.76s | PASS, 20/0/0, 391.00s | PASS, 20/0/0, 268.11s | — | — |
| `rah-tools --lib`, serial/nocapture | PASS, 346/0/0, 1046.77s | PASS, 346/0/0, 837.23s | PASS, 346/0/0, 970.23s | — | — |
| Full `rah-tools` package, serial/nocapture | PASS, 454/0/2 across 18 binaries, summed 1211.36s | PASS, 454/0/2 across 18 binaries, summed 1157.04s | PASS, 454/0/2 across 18 binaries, summed 1183.69s | — | — |

Notation in matrix cells is passed/failed/ignored. The package rows aggregate all test binaries, including the 346-test library binary; its two ignored integration tests require explicitly configured local Cargo/Git executable paths. Command wall durations for the three full package runs were 1215.79, 1164.05, and 1185.13 seconds. No failure-only instrumentation was used; the original failing probe remains unidentified. No run in the bounded matrix failed, and no other intermittent failure appeared.

## Earliest evidenced layer, causality, and disposition

The original failure is real and was observed during the first linked-A staged review. Its earliest **evidenced** layer is Git-layout identity validation, but the particular invocation and earliest failing sublayer among the eight probes, output parsing, layout comparison, and layout capture/currentness remain **unresolved**. The line-1131 panic is only the final test assertion. No fixture command failure, repository selection/admission error, staged diff result, or commit-execution failure was retained in that record. The missing inner error is not invented.

The original failure did **not** reproduce in this bounded matrix: exact test 5/5, linked family 3/3, repository_commit family 3/3, library 3/3, and full package 3/3 all passed. The earlier two Task 423 workspace PASS results and later standalone FAIL establish observed intermittency or context sensitivity, but the clean matrix did not establish an ordering or shared-state trigger. The linked-family runs repeated the immediately preceding test order without failure. Failure-only diagnostics were not authorized by a reproduced failure and were not used. A pre-correction baseline comparison was therefore **not required** and was not run. The correction is semantically active during staged review, but its causal relationship to the original failure remains **unproven**. Neither "correction regression" nor "independent defect" is claimed.

**PASS — LINKED COMMIT GIT-LAYOUT FAILURE DISPOSITIONED AS NON-REPRODUCIBLE.** This is a bounded disposition, not a claim that Task 423's failure never occurred or that future failure is impossible. No production or test source change was required or made; the exact-source disposable worktree remained clean. No retry or sleep was added, and no validation or timeout policy was changed.

Release preparation remains **STOPPED** at Task 423's interrupted standalone package-check sequence. This PASS does not make v0.32.0 READY_FOR_RELEASE. The exact next task is **Task 423B — Resume Remaining v0.32.0 Release Preparation Gates**: verify the exact HEAD and preserved diff, incorporate this disposition, verify an empty Rust/test diff, rerun the standalone rah-tools release check once, finish all remaining standalone package and fmt/check/metadata/diff gates, decide whether established procedure requires another workspace stability run, and only after every gate passes perform the bounded release-preparation commit, clean-tree validation, exact validated-head push, and exact-head CI PASS. Task 423B must not tag or publish. **Task 424 — Publish RAH v0.32.0** remains reserved and cannot start until Task 423B succeeds. Task 423B and Task 424 were not started here.

## Final worktree preservation

The ending main tracked-diff fingerprint remained `66c7d9b0bfc2e27fb66ae79dc041968a4f3e4b67`, matching entry. Final `git diff --check` passed. `git diff -- '*.rs'` and `git diff --cached --name-only` were empty. HEAD remained `83c5c30446a2f566be7b660d45e6d70b98a8d344`. The final dirty set was exactly the six original tracked paths and four untracked paths: the original three Task 423 release-preparation files plus this Task 423A artifact. The pre-existing Task 423 release-preparation content remains unchanged. The clean disposable worktree was removed; the dedicated Cargo target and complete per-run logs were retained outside the main repository for evidence. This artifact is left uncommitted in the dirty release-preparation worktree so no release-preparation paths need to be staged or committed during Task 423A. No push, tag, or publication occurred.
