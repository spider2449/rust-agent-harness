# Task 421 — Linked File-Info Intermittent Failure Disposition

Date: 2026-09-25

## Starting checkpoint and frozen scope

Committed HEAD: `8c5f47158cb61a02d7285dda208e3e4d52fb9526` (`docs: localize staged observation timeout semantics`). The index was empty and `git diff --check` passed with existing line-ending warnings. The dirty paths were the eight frozen Task 412 release-preparation files (`CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/RAH_V0.32_RELEASE_GATE.md`, and `docs/plans/2026-09-25-task-412-v0.32-release-preparation.md`), three corrected Rust files (`repository_diff.rs`, `repository_observer.rs`, `repository_git_layout.rs`), the Task 420 plan, and the pre-existing untracked Task 421 draft. The Task 421 draft already existed at task start and was completed in place. The frozen Rust and Task 412 binary-diff Git object fingerprints were `ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3` and `d3250597309d0ae38cd325b5b13ac060c94169be`, respectively.

Task 420 stopped after the second full `rah-tools` package run reported `repository_file_info::tests::linked_file_info_reads_the_selected_worktree_file_and_head` failed. Its complete panic and package counts were not retained. This task investigates that failure without changing production or test source, Task 420 semantics, or Task 412.

## Test and production path

The exact test is `crates/rah-tools/src/repository_file_info.rs:681-727`. `WorktreeFixture::new` in `repository_git_layout.rs:793-866` creates one temporary parent with a `main` repository and two real Git linked worktrees, `linked-a` and `linked-b`. Its name includes process ID, an atomic sequence, and a UUID. The fixture configures a local Git identity and `core.autocrlf=false`, commits `tracked.txt` with `initial\n` in `main`, and creates both linked branches from that commit. The test writes `A has a distinct committed file\n` to `linked-a/tracked.txt`, stages and commits it in A, constructs `RepositoryFileInfoTool` for A, and requests `tracked.txt`. It requires JSON `status:ok`, a worktree byte size equal to the selected byte string, a selected HEAD tree file object ID different from B's `HEAD:tracked.txt`, and B's file bytes still equal to `initial\n`. The test does not compare the selected HEAD commit ID directly; the differing file object ID proves the selected committed file differs from B. The fixture `Drop` removes its own temporary parent after the test ends.

Call graph: test → `RepositoryFileInfoTool::new` → `RepositoryObserver::new` → `RepositoryIdentity::capture` → `RepositoryGitLayout::capture` (including linked gitfile, private/common directory, backlink, commondir and registration evidence). Execution is test → `RepositoryFileInfoTool::execute` → lease and repository/target revalidation → one `Instant::now()` → `RepositoryObserver::run` for `Index`, `Head`, `HeadTree`, and `FileInfoStatus` in that order → `run_with_budget(..., None)` for each → repository/layout revalidation → `validate_git_with_budget(..., None)` → eight fixed Git layout probes → `child_timeout(FILE_INFO_TIMEOUT, started, None, now)` → one fixed observer Git child. The tool then parses index, HEAD, tree, and status, directly observes the selected worktree file, normalizes, and returns JSON. Fixed child commands are `ls-files --stage -v -z --full-name --no-abbrev`, `rev-parse --verify -q HEAD`, `ls-tree -z -l HEAD`, and porcelain-v2 path status. Git layout probes are top-level, absolute private Git dir, common Git dir, bare state, superproject, index path, HEAD path, and worktree registration list.

The file-info contract is one five-second elapsed budget measured from just before the first `Index` observation and shared through all four observer children. Each of the eight layout probes per observation retains a five-second child ceiling; these probes are not assigned a separate aggregate by file-info. `DIFF_TIMEOUT=15s` applies to diff paths only. Task 059 explicitly introduced the five-second aggregate file-info observer; Task 060 named it `FILE_INFO_TIMEOUT`. Task 417's aggregate probe clamp applies only when an aggregate is supplied. File-info calls `run`, not `run_diff`, and supplies `None`. Task 420's `child_timeout` helper is invoked, but its `None` branch uses the original `5s - elapsed` rule.

## Committed-source comparison and static impact

`repository_file_info.rs` and its test are **UNCHANGED** from the committed HEAD. `repository_diff.rs` is outside this call path. In `repository_observer.rs`, `run` now delegates to `run_with_budget(..., None)` and the identity/layout calls accept that optional value: **signature/plumbing changes** for file-info. The old `checked_sub(started.elapsed())` plus zero rejection and the new `saturating_sub(now - started)` plus zero rejection are **semantically equivalent** for file-info, including at and after five seconds. `RepositoryGitLayout::validate_git`/`probe` now pass an optional aggregate: **signature/plumbing changes** here. With `None`, `probe_timeout` returns the unchanged `PROBE_TIMEOUT=5s`, and the new aggregate-expiry error branch cannot run. No changed Task 417/420 semantics are active on this path. The staged HEAD composition correction is therefore structurally independent of this file-info result.

## Linked-worktree lifecycle and shared state

The selected A `.git` file points to A's private directory under the shared common `.git/worktrees` registry. The private directory carries A's `HEAD`, `index`, `commondir`, and `gitdir` backlink; Git's eight Task 359 probes verify those identities and the selected registration. B has a different private directory, HEAD/index and file, while sharing the common Git dir and object store. The fixture commits A before constructing the tool and removes the fixture after the tool and assertions finish. The code does not spawn an unowned fixture process: its setup commands use `std::process::Command::output`, and observer Git children are awaited through `execute_process`.

For suite interference, the observed code uses a per-root `OnceLock` lease map keyed by canonical selected root, with weak references; unique fixture roots keep this test's lease separate. The fixture resolves native `git.exe` with `where.exe`, canonicalizes it, and the observer revalidates executable identity. Observer children use a fixed cwd at the selected root and an explicit, sanitized Git environment; no file-info or fixture code changes process cwd. The fixture's temporary name includes a UUID, and its `Drop` removes only its own parent. These facts rule out obvious fixture-name and lease collisions; they do not by themselves prove absence of every suite interaction.

## Reproduction matrix

Corrected disposable worktree: `F:\Temp\rah-task421\corrected`, detached at the checkpoint HEAD, with exactly the three-file Rust patch applied. Its binary-diff fingerprint was `d3250597309d0ae38cd325b5b13ac060c94169be`. Cargo target: `F:\Temp\rah-task421\target-corrected`, separate from Task 418/419 targets.

| Scope | Run | Result | Counts | Wall duration | Evidence |
| --- | ---: | --- | --- | ---: | --- |
| Exact linked-file test | 1 | PASS | 1 passed | 35.60s including fresh compile; test 3.30s | `focused-1.log` |
| Exact linked-file test | 2 | PASS | 1 passed | 5.04s; test 4.56s | `focused-2.log` |
| Exact linked-file test | 3 | PASS | 1 passed | 3.61s; test 3.27s | `focused-3.log` |
| Exact linked-file test | 4 | PASS | 1 passed | 3.58s; test 3.29s | `focused-4.log` |
| Exact linked-file test | 5 | PASS | 1 passed | 3.51s; test 3.20s | `focused-5.log` |
| File-info family | 1 | PASS | 4 passed | 3.82s; tests 3.43s | `family-1.log` |
| File-info family | 2 | PASS | 4 passed | 3.44s; tests 3.16s | `family-2.log` |
| File-info family | 3 | PASS | 4 passed | 3.65s; tests 3.34s | `family-3.log` |
| `rah-tools --lib` | 1 | PASS | 346 passed, 0 failed, 0 ignored | 978.65s; tests 978.29s | `lib-1.log` |
| `rah-tools --lib` | 2 | PASS | 346 passed, 0 failed, 0 ignored | 972.67s; tests 972.10s | `lib-2.log` |

The focused runs used `--exact --test-threads=1 --nocapture`; family runs used `--test-threads=1 --nocapture`. In the family runs the linked test ran first and passed, so no preceding file-info test was required for the observed clean outcome.



## Remaining library and full-package matrices

| Scope | Run | Result | Counts | Wall duration | Evidence |
| --- | ---: | --- | --- | ---: | --- |
| `rah-tools --lib` | 1 | PASS | 346 passed, 0 failed, 0 ignored | 978.65s; tests 978.29s | `lib-1.log` |
| `rah-tools --lib` | 2 | PASS | 346 passed, 0 failed, 0 ignored | 972.67s; tests 972.10s | `lib-2.log` |
| `rah-tools --lib` | 3 | PASS | 346 passed, 0 failed, 0 ignored | 939.67s test time; wall duration not separately retained | `lib-3.log` |
| Full package | 1 | PASS | 454 passed, 0 failed, 1 ignored; 0 doc tests | 991.18s | `package-1.log` |
| Full package | 2 | PASS | 454 passed, 0 failed, 1 ignored; 0 doc tests | 965.69s | `package-2.log` |
| Full package | 3 | PASS | 454 passed, 0 failed, 1 ignored; 0 doc tests | 1165.20s | `package-3.log` |

`cargo test -p rah-tools --lib -- --test-threads=1 --nocapture` was run three times. Library run 3 reports 346 passed, 0 failed, 0 ignored, 936.52s test time; a separate process wall timer was not retained. Each package invocation used `cargo test -p rah-tools -- --test-threads=1 --nocapture`, and each completed with exit code 0. Per invocation the library reported 346 passed; integration binaries reported 108 passed and one ignored (`explicitly_configured_local_git_reports_temporary_repository_status`, requiring `RAH_GIT_STATUS_EXECUTABLE`); doctests reported zero tests. No different intermittent failure appeared.

The original Task 420 package failure was not reproduced in the five exact focused runs, three file-info family runs, three library runs, or three complete package runs. The failure's original panic/counts remain unavailable, so those historical details are not inferred.

## Baseline, diagnostics, and isolation findings

A clean baseline worktree comparison was **not required**: the corrected implementation did not reproduce the failure. No temporary diagnostics were used. No baseline commands were run. The earliest failing layer cannot be assigned because no failure recurred; the reported failure remains unexplained rather than disproven.

The call path was inspected for suite interference. Its fixture uses `std::env::temp_dir()` plus process ID, an atomic per-process sequence, and UUID, so paths are unique across runs and processes. Fixture Git setup uses synchronous `Command::output()` and assertions require success; fixture teardown removes only its own temporary parent. The observer's per-root lease map is keyed by canonical repository root and holds weak references; this test creates a fresh root. The file-info and layout code do not mutate process CWD or use mutable global Git environment. Observer children use fixed selected-root cwd and a cleared/host-built Git environment, and are awaited via the supervised `execute_process` path. These checks found no evident suite-order/shared-state trigger. They cannot establish the absence of all external runtime interference.

The linked fixture has one common main repository with two registered linked worktrees. Each linked root's `.git` file names a private worktree Git directory. The private directory has its own `HEAD` and `index`, `commondir` to shared common metadata, and `gitdir` backlink; the common `.git/worktrees` registry records both. Git layout validation checks top-level, private/common Git directory, bare and superproject state, index path, HEAD path, and `worktree list --porcelain -z`. The test's pre-execution commit on A and final read of B make the selected-worktree distinction observable. No external process is detached: fixture commands are synchronously collected, observer commands are awaited, and fixture teardown follows completion.

## Causality and disposition

Classification: **currently non-reproducible within the required corrected-worktree matrix**. The static path analysis shows the Task 420 staged HEAD formula is not active for `repo.file-info`: file-info uses `RepositoryObserver::run`, passes no aggregate deadline, and the helper's non-aggregate branch retains elapsed-per-child accounting. Task 417's probe-deadline clamp likewise is inactive because layout validation receives `None`; the eight probes retain their five-second per-child ceiling. Therefore the observed chronology does not establish causality, and the Task 420 correction has no structural semantic route to this reported file-info symptom.

No Rust source or test source changed. No Task 412 release-preparation file or Task 420 correction changed. No commit, push, or tag occurred. No files are staged. Task 421 remains uncommitted alongside the Task 420 record for the later correction audit.

Verdict: **PASS � LINKED FILE-INFO PACKAGE FAILURE DISPOSITIONED AS NON-REPRODUCIBLE**. This does not claim the original failure never occurred. The five focused runs, three file-info family runs, three library runs, and three consecutive full-package runs all passed. No source change is required based on this evidence.

Task 420 may proceed to independent audit. Recommended next task: **Task 422 � Shared Staged Deadline and HEAD Timeout Composition Independent Audit**, covering Task 416 aggregate policy, Task 417 probe-deadline enforcement, Task 419 HEAD semantic defect, Task 420 child-timeout formula, and independent review of the stress evidence.
