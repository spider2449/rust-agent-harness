# Task 446 — Repository commit parallel-gate causality audit

Status: **F — REPOSITORY COMMIT PARALLEL FAILURE REMAINS UNLOCALIZED**. Task 445 remains **STOPPED / NOT CERTIFIED**. Task 438 remains paused. No Task 445 patch was exported or integrated.

## Preserved original failure evidence

The Task 445 plan records a single normal-parallel `cargo test -p rah-tools` failure: 345 passed, 3 failed. The original complete command output was not found in the Task 445 worktree or the available `F:\Temp` top-level logs. Consequently panic locations, exact first failed Git commands, child status, and probe details cannot be recovered from that run. The following names and terse results are transcribed from `2026-09-27-task-445-repository-file-info-budget-composition.md`, not guessed from source.

| Exact failed test | Panic location | First failed operation / actual result in preserved record | Phase known from original record | Git child timed out | Semantic validation failed | Output overflow |
| --- | --- | --- | --- | --- | --- | --- |
| `repository_commit::tests::invalid_messages_and_changed_index_refuse_before_spawn` | Not retained | “bounded repository HEAD observation”; exact child/result not retained | Not retained; test source invokes authorization and commit precondition checks | Unknown | Unknown | Unknown |
| `repository_commit::tests::opaque_review_is_refused_by_another_policy` | Not retained | “observation total timeout”; exact observation stage not retained | Not retained; test source invokes review | Unknown | Unknown | Unknown |
| `repository_commit::tests::policy_generation_head_and_index_races_refuse_before_spawn` | Not retained | “observation total timeout”; exact observation stage not retained | Not retained; test source invokes authorization and commit precondition checks | Unknown | Unknown | Unknown |

An observation total timeout does not itself prove a Git child timed out. It can be reached before the child is spawned, after successful semantic probes have consumed the logical operation budget.

## Task 445 shared-path audit

The full Task 445 `git diff` was inspected before tests. Changed production functions are `RepositoryFileInfoTool::execute` (four calls), `RepositoryObserver::new` (constant rename only), `RepositoryObserver::run_file_info` (new), `RepositoryObserver::run_with_budget` (constant rename only), and `file_info_budget` (new). Added tests in `repository_git_layout.rs` and `repository_observer.rs` exercise the file-info budget. No production code changed in `RepositoryIdentity::validate_git`, `RepositoryIdentity::validate_git_with_budget`, `RepositoryGitLayout::validate_git`, or `RepositoryGitLayout::validate_git_with_budget`. The latter still receives `None` from plain `validate_git`; commit calls the plain method before each command. Classification: **file-info/observer-specific only**. A deterministic unit comparison of unbudgeted semantics was unnecessary because the shared validator bodies and plain call chain are unchanged.

## Exact isolated matrix

`RUST_BACKTRACE=1`; each invocation used `cargo test -p rah-tools repository_commit::tests::<exact name> -- --exact --nocapture --test-threads=1`. Each was run exactly three times per tree, in test-name order. Individual logs are `F:\Temp\rah-task446-dirty-<name>-<1..3>.log` and `F:\Temp\rah-task446-clean-<name>-<1..3>.log`.

| Test suffix | Task 445 WIP | Clean base |
| --- | ---: | ---: |
| `invalid_messages_and_changed_index_refuse_before_spawn` | 3/3 PASS | 3/3 PASS |
| `opaque_review_is_refused_by_another_policy` | 3/3 PASS | 3/3 PASS |
| `policy_generation_head_and_index_races_refuse_before_spawn` | 3/3 PASS | 3/3 PASS |

The clean comparison worktree was created at `F:\Temp\rah-task446-clean-base`, detached at exact HEAD `deba6a10d65ce5db9842a77db3420a89cc0201ec`; `git status --short` was empty before and after its runs.

## Normal-parallel comparison

- Clean base: exactly one `cargo test -p rah-tools -- --nocapture`, log `F:\Temp\rah-task446-clean-rah-tools.log`, exit 0. Library result: **346 passed, 0 failed** in 195.07 seconds. All three target tests passed. The remaining package test binaries passed.
- Task 445 WIP: one diagnostic invocation of the same command, with `RUST_BACKTRACE=1`, log `F:\Temp\rah-task446-task445-rah-tools.log`, exit 101 **during compilation**. No tests ran. Temporary timing output attempted to format `ObserverCommand` with `Debug`, which that type does not implement (`E0277`, three errors). The temporary instrumentation in `repository_commit.rs` and `repository_observer.rs` was restored from byte-for-byte backups immediately. The diagnostic command was not retried under the task's one-run limit. The resulting log is a preparation failure, not evidence of a Task 445 runtime failure.

## Execution model and timing limits

`RepositoryCommitPolicy::capture_snapshot` explicitly validates the repository once, then uses seven ordinary Git commands: `symbolic-ref`, two `rev-parse`, `config`, `ls-files`, `write-tree`, and `diff`. Each `run()` again performs `RepositoryIdentity::validate_git()` before constructing the fixed execution policy and spawning its actual child with a **5-second child ceiling**. `run_commit()` similarly validates before the actual commit child, whose ceiling remains **15 seconds**. Each successful semantic validation currently launches eight fixed Git probes. The staged `IndexVsHead` diff used in review and authorization has five observation children: HEAD, raw, numstat, patch, HEAD; each has its own validation.

For a successful `review()` path, this source sequence implies 21 semantic validations (8 before/within the first snapshot, 5 in the staged diff, 8 before/within the second snapshot), up to 168 validation probes, and 19 actual observation children, aside from fixture setup. A successful `authorize()` path implies 13 validations, up to 104 probes, and 12 children. These are **source-derived path counts**, not dynamic measurements of the original failed runs; early errors shorten the sequence. There is no single 5-second total budget for repository commit. The attempted timing diagnostic produced no validation or child durations because compilation failed.

## Disposition

Task 445 did not change the shared commit validator path. Both trees passed all isolated target tests, and the clean normal-parallel run passed. The historical Task 445 normal-parallel run failed, but its detailed output is unavailable and the one WIP diagnostic attempt did not execute tests. This evidence does not prove a Task 445 regression, pre-existing normal-parallel instability, a production validation/process-pressure defect, or a test fixture defect. In particular, it does not distinguish validation failure, actual child timeout, and successful-but-slow validation. **F — unresolved** is the supported classification.

Task 445 may not continue to certification or integration from this audit. No commit timeout, validation, authority, fixture, or production correction was made. The adjacent worktree-diff budget observation remains in the Task 445 plan and outside this audit. HostExplicit remains exactly 11; no v0.33 product capability was selected; RAH remains 0.32.0. No commit, push, patch export, or resumption of Tasks 441/438 occurred.
