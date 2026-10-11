# Task 414 — Intermittent Desktop Git Observation Failure Isolation

Date: 2026-09-25
Status: **PASS — INTERMITTENT DESKTOP GIT OBSERVATION FAILURE ROOT CAUSE ISOLATED**
Task type: reproduction and isolation; no behavior correction

## Verdict

```text
PASS — INTERMITTENT DESKTOP GIT OBSERVATION FAILURE ROOT CAUSE ISOLATED
```

The reproduced failure mechanism is exhaustion of the shared 15-second
`IndexVsHead` observation deadline. In the failing serial Desktop suite, the
staged-diff path returned:

```text
Git repository policy rejected capability: repository observation exceeded its total timeout
```

The timeout is checked in `RepositoryObserver::run` after repository Git-layout
validation and before the next fixed Git command is spawned. The staged snapshot
uses one `Instant` across its pre-HEAD check, raw diff, numstat diff, patch diff,
and post-HEAD check. Every one of those `run` calls repeats eight Git-layout
probe processes. The aggregate work can exhaust the single 15-second budget in
full-suite context.

This explains the missing Stage action and `StagedDiffExecution` snapshot
failures: the snapshot fails before action derivation, and refresh publishes
its existing empty `ReviewUnavailable` fallback. The review-digest test's
historical empty-vector panic is mechanically compatible with that same
fallback, but that test passed in every Task 414 focused and package run. The
OS/filesystem scheduling factor that caused the aggregate work to exceed 15
seconds was not isolated.

Freeze this narrow Task 415; do not implement it as part of Task 414:

```text
Task 415 — Correct staged-diff aggregate timeout exhaustion while preserving
the fixed Git commands, repository identity checks, and selector/currentness
contracts.
```

## Starting checkpoint and frozen Task 412 state

Starting committed HEAD:

```text
c1ba17717f0eff66b46b5c4d5593c81d009480c0
docs: disposition v0.32 release validation failures
```

The main worktree was `DIRTY` with exactly these Task 412 release-preparation
paths:

```text
CHANGELOG.md
Cargo.lock
Cargo.toml
README.md
docs/ARCHITECTURE.md
docs/SECURITY.md
docs/RAH_V0.32_RELEASE_GATE.md
docs/plans/2026-09-25-task-412-v0.32-release-preparation.md
```

At Task 414 start, and again immediately before creating this artifact,
`git diff --binary | git hash-object --stdin` returned:

```text
ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3
```

The main worktree's `git diff --check` passed. `git diff -- '*.rs'` was empty.
Task 412 paths were not changed during the research.

## Task 413 evidence carried forward

Task 413 established:

- The two originally reported Task 412 tests passed three focused runs each;
  the Desktop package suite passed once.
- A serial workspace run reported 317 passed, 3 failed, 18 ignored, 338
  discovered. The failures were `old_repository_selector_cannot_resolve_to_new_repository_action`,
  `repository_snapshot_matrix_isolated_repositories_and_replacements`, and
  `review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors`.
- The snapshot-matrix failure reported `StagedDiffExecution`; the review-digest
  failure indexed an empty staged-diff vector.
- The snapshot and review-digest failure modes also appeared on the clean
  pre-release baseline. No version or dependency causality was established.
- Task 412 remained blocked. No release-validation work was resumed here.

## Primary cluster, control, and exact source locations

All four functions are in `crates/rah-desktop/src/main_tests.rs` in the
`tests` module. Their original source ranges at starting HEAD are:

| Label | Test | Source range | Result across Task 414 |
|---|---|---:|---|
| A | `old_repository_selector_cannot_resolve_to_new_repository_action` | 2900–2970 | Focused 5/5 pass; failed in package runs 1 and 3 at “A exposes one stage action”; passed in package run 2. Run 3 logged the shared timeout. |
| B | `repository_snapshot_matrix_isolated_repositories_and_replacements` | 2182–2296 | Focused 5/5 pass; failed in package runs 1 and 3 at `StagedDiffExecution`; passed in package run 2. Run 3 logged the shared timeout. |
| C | `review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors` | 2973–3010 | Focused 5/5 pass and passed in all three package runs; no Task 414 failure reproduced. |
| D control | `observed_single_target_stage_and_unstage_refresh_and_consume_selectors` | 2838–2897 | Focused 3/3 pass; failed in package runs 1 and 3 with the empty refresh fallback; passed in package run 2. Run 3 logged the shared timeout. |

`cargo test -p rah-desktop -- --list` emitted 338 tests. In that list D was
ordinal 186, A 187, B 226, and C 235. In the serial package output, D immediately
preceded A; B followed `repository_snapshot_classifies_first_observer_execution_failure`;
C followed `resumed_conversation_imports_exact_pairs_and_next_request_appends_prompt`.
The same relevant test ordering appeared in each package run. No preceding
test was shown to mutate a shared repository fixture or selector cache.

## Production call graph

The relevant flow is:

```text
TestRepository::git_repository
  -> native Git init/config/add/commit and selected fixture state
  -> DesktopRepository::new(git, fixture root)
  -> desktop_repository_snapshot_with_review
       -> RepositoryStatusTool
       -> RepositoryDiffTool (worktree vs index)
       -> RepositoryDiffStagedTool (index vs HEAD)
            -> execute_fixed_diff(IndexVsHead)
                 -> pre-HEAD probe
                 -> raw staged diff
                 -> numstat staged diff
                 -> patch staged diff
                 -> post-HEAD probe
  -> desktop_snapshot normalization
  -> prepare_repository_workflow
       -> Stage / Unstage eligibility and action entries
       -> staged review digest
  -> publish_repository_workflow
       -> opaque selectors and observation generation
  -> repository_index_action / begin_repository_index_effect
       -> action kind, repository generation, observation generation,
          member and repository-pointer currentness
```

For A and D, the tests call `refresh_repository_workflow`, which first awaits
the complete snapshot. On an observation error it constructs an empty
`RepositorySnapshot` with `ReviewUnavailable { reason: "bounded staged
observation failed" }`; the empty status vector then yields no Stage action.
The error occurs before selector creation or stale-selector validation.

For B, `desktop_repository_snapshot` is called directly. It returns the
specific private `RepositoryObservationStage::StagedDiffExecution` error when
the staged tool fails. In Task 414 run 1 the matrix's first A snapshot failed;
in run 3 its B snapshot failed. This variation is consistent with a time
budget boundary rather than a stable repository-specific fixture assumption.

For C, refresh returns a successful fallback snapshot after an observer error,
so `first.staged_diff[0]` or `second.staged_diff[0]` would panic if that
fallback were empty. C did not fail in Task 414, so that path is an explanation
of the historical symptom, not a newly reproduced C event.

Relevant production symbols and original line ranges:

- `DesktopRepository::new` constructs the status, worktree-diff, and
  staged-diff tools: `crates/rah-desktop/src/main.rs:1203–1279`.
- `RepositoryObservationStage::StagedDiffExecution` and
  `desktop_repository_snapshot_with_review`: `crates/rah-desktop/src/main.rs:5154–5166`,
  `5390–5444`.
- `RepositoryDiffStagedTool::execute` routes to `execute_fixed_diff` with
  `DiffBaseline::IndexVsHead`:
  `crates/rah-tools/src/repository_diff_staged.rs:22–64`.
- One lease and one `Instant` span staged diff phases:
  `crates/rah-tools/src/repository_diff.rs:84–135`.
- `RepositoryObserver::run` validates repository identity, checks remaining
  time, then selects and executes its fixed command:
  `crates/rah-tools/src/repository_observer.rs:332–391`.
- `prepare_repository_workflow` derives Stage/Unstage actions and the staged
  digest: `crates/rah-desktop/src/main.rs:5522–5626`.
- `publish_repository_workflow` publishes action IDs and currentness
  generations: `crates/rah-desktop/src/main.rs:5629–5740`.
- `refresh_repository_workflow` converts observer errors to the empty fallback
  and then checks repository pointer, generation, and active member before
  publication: `crates/rah-desktop/src/main.rs:5774–5891`.
- `replace_selected_repository` changes selected repository state and resets
  workflow state: `crates/rah-desktop/src/main.rs:5948–5974`.
- `begin_repository_index_effect` validates action kind and repository and
  observation generations: `crates/rah-desktop/src/main.rs:6389–6510`.
- `repository_index_action` dispatches only after that reservation:
  `crates/rah-desktop/src/main.rs:7856–7928`.

## `StagedDiffExecution` mapping and timeout evidence

`RepositoryObservationStage` is a private test/diagnostic classification. It
does not carry command path, repository path, exit code, stdout, or stderr.
`desktop_repository_snapshot_with_review` maps every `ToolError` from the
staged tool to `StagedDiffExecution`, discarding its detail. The frontend sees
only the sanitized `ReviewUnavailable` fallback on refresh.

The decisive timeout path is:

- `DIFF_TIMEOUT` is 15 seconds in `crates/rah-tools/src/repository_observer.rs`
  (constant near lines 28–35).
- `execute_fixed_diff_while_leased` starts one timer at line 102 and reuses it
  for both HEAD observations and all three diff phases at lines 103–127.
- Before each observer command, `RepositoryObserver::run` performs `revalidate`,
  `repository.validate_git`, and observation-boundary validation at lines
  339–361. `repository.validate_git` in
  `crates/rah-tools/src/repository_git_layout.rs:358–433` invokes eight fixed
  Git probes: `rev-parse --show-toplevel`; `rev-parse --path-format=absolute
  --absolute-git-dir`; `rev-parse --path-format=absolute --git-common-dir`;
  `rev-parse --is-bare-repository`; `rev-parse
  --show-superproject-working-tree`; `rev-parse --path-format=absolute
  --git-path index`; `rev-parse --path-format=absolute --git-path HEAD`; and
  `worktree list --porcelain -z`.
- Only after those checks does `run` subtract `started.elapsed()` at lines
  374–380. It returns `repository observation exceeded its total timeout` if
  the shared budget is exhausted. At that branch the next fixed command has
  not been spawned.

The no-commit-control staged path therefore performs up to five observer
commands (pre-HEAD, raw, numstat, patch, post-HEAD), each preceded by eight
Git-layout probes, under one 15-second total budget. The source proves this
budget and sequence; Task 414 did not measure per-probe durations.

### Fixed staged Git diff commands

The exact observer arguments are:

```text
--no-pager diff --cached --raw -z --no-abbrev --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
--no-pager diff --cached --numstat -z --no-renames --no-ext-diff --no-textconv --ignore-submodules=all --submodule=short
--no-pager diff --cached --patch --no-color --no-prefix --full-index --no-renames --no-relative --no-ext-diff --no-textconv --diff-algorithm=myers --no-indent-heuristic --inter-hunk-context=0 --unified=3 --ignore-submodules=all --submodule=short
```

The HEAD probe command is `--no-pager rev-parse --verify -q HEAD`. Git layout
validation uses the eight commands listed above. The selected test executable
was `C:\Program Files\Git\cmd\git.exe` (`where.exe git.exe` first result,
canonicalized by the fixture helper).

No `git diff --cached` process was captured exiting nonzero. In the reproduced
run-3 failures, the emitted error was the observer's aggregate timeout. Because
it is checked before the next fixed command is spawned, there is no failing
Git command's exit code or stderr to report. The exact next command phase was
not logged. The earlier Git probes and fixed commands had returned far enough
for this timeout guard to be reached, but their individual durations and
outputs were not recorded. The uninstrumented sanitized stage alone would not
have distinguished this from other staged-tool errors; the temporary
test-binary diagnostic captured the underlying timeout message.

## Git command semantics evidence

Task 414 executed the configured diff shapes against a disposable ordinary
repository with Git 2.55.0.windows.5. Results:

| State | `git diff --cached` exit/output | Evidence |
|---|---|---|
| Fresh initialized repository with empty index and no HEAD | Exit 0, empty for raw, numstat, and patch forms | Direct disposable Git check. |
| Fresh initialized repository with a staged new file and no HEAD | Exit 0, patch output present | Direct disposable Git check. |
| Committed clean repository | Exit 0, empty for all three forms | Direct disposable Git check. |
| Worktree-only edit, index unchanged | Exit 0, empty for cached forms; ordinary `git diff --patch` had output | Direct disposable Git check. |
| Staged tracked-file edit | Exit 0, output present for raw, numstat, and patch forms | Direct disposable Git check. |
| Immediately after unstage/reset | Exit 0, cached output empty | Direct sequential check; no concurrent mutation. |
| Linked worktree with a staged edit | Exit 0, linked worktree showed the staged patch; main worktree stayed staged-clean | Direct disposable Git check. |
| Repository replacement | The tests use separately initialized repositories and bind each observer to the newly selected root; no Git command failure attributable to replacement was observed. | A, B, and the snapshot matrix source. |

These commands do not use `--exit-code`; a nonempty diff is a successful
observation, and an empty diff is also a successful observation. Unstaged
worktree changes alone do not make the index-vs-HEAD diff nonempty. The
disposable checks did not simulate a concurrent index writer or `index.lock`.

## Fixture lifecycle and process/global-state audit

`TestRepository::new` uses `std::env::temp_dir()` and a nanosecond timestamp
plus process-local `NEXT_TEST_DIRECTORY` counter for each normal fixture root
(`main_tests.rs:303`, `406–420`). The normal helper does not include a process
ID. `git_repository` removes placeholder files, runs `git init --quiet`, sets
local `user.email` and `user.name`, writes and commits a baseline, then creates
one of Clean, Untracked, Modified, or Staged states (`main_tests.rs:461–531`).
`Drop` calls `remove_dir_all` (`main_tests.rs:534–537`). The primary tests use
ordinary repositories, not linked worktrees, and do not share their Git index,
objects, root, `DesktopAppState`, or repository object. Linked worktree
semantics were checked separately as described above.

The fixture helper locates Git with `where.exe git.exe`, takes the first
reported path, and canonicalizes it (`main_tests.rs:461–477`). Fixture
initialization inherits the test process environment and system/global Git
configuration; it sets identity locally but does not set `core.autocrlf` in
this helper. The observer process uses `env_clear` and a host-fixed environment
that disables system/global config, fsmonitor, and untracked cache, sets
`GIT_OPTIONAL_LOCKS=0`, disables terminal prompting, and supplies only the
selected repository's `safe.directory` value (`crates/rah-tools/src/git_support.rs:9–45`).

The relevant tests do not call `set_current_dir`, `set_var`, or `remove_var`.
They set child `current_dir` explicitly. A search found environment mutation
only in an ignored host-only Git discovery probe, which did not run. The only
relevant shared fixture counter is `NEXT_TEST_DIRECTORY`; no shared selector
cache or test-wide repository singleton was found. `--test-threads=1` was used
for every requested invocation.

The mutation helpers synchronously wait for their Git process via
`Command::output()`. Production host Git calls use `execute_host_process`,
which waits for child exit, joins its two bounded output-reader tasks, and owns
termination on timeout/drop (`crates/rah-sandbox/src/supervised_process.rs:83–205`).
The staged diff owns a repository lease across its fixed phases. Each phase is
awaited; no detached observation task or delayed publication was found in this
path. Refresh publication rechecks selected repository pointer, repository
generation, and active member after observation. No evidence showed a stale
repository owner being published.

No direct evidence implicated antivirus, indexing, a filesystem filter, or
index locking. No such cause is claimed.

## Disposable investigation worktree

The clean worktree was created from the requested committed source:

```text
commit: c1ba17717f0eff66b46b5c4d5593c81d009480c0
path: F:\coding\otherPrj\rust-agent-harness-task414
```

All repeated tests and temporary diagnostics ran there. The initial Desktop
package run found the explicitly declared provider test binaries missing:
`rah-plugin-echo.exe` and `rah-mcp-echo-server.exe`. The test helper asserts
those fixtures exist (`crates/rah-desktop/src/provider_composition.rs:180–197`).
The first run's three provider-composition failures were therefore a test
prerequisite miss, not part of the Git observation cluster. Before package
runs 2 and 3, the declared fixtures were built with:

```text
cargo build -p rah-tools-mcp -p rah-tools-plugin
```

The diagnostic source edits were fully reverted. The disposable worktree's
tracked source blobs match HEAD and its worktree is clean before removal.

## Focused stress matrix

Each command used `--exact --test-threads=1 --nocapture`; each run was a
separate invocation. No source changed during the required initial matrix.

| Label | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 |
|---|---|---|---|---|---|
| A selector/replacement | PASS, 16.07s | PASS, 16.00s | PASS, 17.19s | PASS, 16.61s | PASS, 15.91s |
| B snapshot matrix | PASS, 39.67s | PASS, 40.04s | PASS, 39.72s | PASS, 40.02s | PASS, 40.34s |
| C review digest | PASS, 14.77s | PASS, 14.08s | PASS, 14.02s | PASS, 14.26s | PASS, 15.06s |

D control, three required runs:

| Run 1 | Run 2 | Run 3 |
|---|---|---|
| PASS, 24.19s | PASS, 24.17s | PASS, 25.24s |

Each invocation ran one test and filtered 337. No focused panic or Git error
occurred. Complete logs are retained outside the repository in
`F:\Temp\task414-focus\` (`A-1.log` through `D-3.log` and `summary.txt`).

## Desktop package suite reproduction

The emitted test list contained 338 tests. All three bounded package invocations
used:

```text
cargo test -p rah-desktop -- --test-threads=1 --nocapture
```

| Run | Fixture state | Result | Relevant observations |
|---|---|---|---|
| 1 | Provider helper executables were absent. | FAIL: 313 passed, 7 failed, 18 ignored; harness 854.06s, command 854.80s. | Three declared provider fixtures failed to load. `disconnect_revokes_pending_authorization_and_never_restores_old_review` reported `StagedDiffExecution`; D had an empty fallback with no Stage action; A failed “A exposes one stage action”; B failed “A snapshot succeeds: StagedDiffExecution”. C passed. |
| 2 | Provider helper executables built; first temporary logger active. | FAIL: 316 passed, 4 failed, 18 ignored; harness 892.37s, command 956.99s including rebuild. | A, B, C, and D all passed. `deletion_host_prepare_path_is_zero_effect_complete_and_private`, `deletion_hostexplicit_post_started_failures_are_uncertain_without_replay`, `deletion_hostexplicit_rejects_stale_before_started_for_git_and_desktop_drift`, and `deletion_hostexplicit_success_dispatches_once_and_invalidates_commit_review` failed at staged review preparation; the logger did not preserve their underlying `ToolError`. |
| 3 | Provider helper executables built; Desktop test-only error/stage logger active. | FAIL: 317 passed, 3 failed, 18 ignored; harness 860.61s, command 929.64s including rebuild. | D, A, and B all failed with the exact shared 15-second total-timeout error. C passed. |

The run-3 `TASK414_TOOL_ERROR` and `TASK414_OBSERVATION_STAGE` diagnostics occur
immediately before D, A, and B's assertions. The diagnostic also logged one
`StatusExecutionOrRevalidation` error inside the passing
`repository_snapshot_classifies_first_observer_execution_failure` test; that
test deliberately verifies failure classification and is not an additional
intermittent failure.

Complete package logs are retained in `F:\Temp\task414-package\`:
`desktop-1.log`, `desktop-2-diagnostic.log`, `desktop-3-diagnostic.log`, and
`summary.txt`.

No fourth package run was made. The package runs did not remain clean, so the
conditional workspace-context run was not appropriate and was not run. No
Task 412 workspace validation was resumed.

## Post-failure Git state and exact child-process evidence

The primary fixtures are removed by `TestRepository::drop` during panic
unwinding. Immediately after Task 414 run-1 D and run-3 D failed, the reported
temporary fixture paths were checked. Both had already been removed. Therefore
these requested read-only commands could not be run against those failed
fixtures:

```text
git status --porcelain=v2
git diff --cached --name-status
git diff --cached --raw
git ls-files --stage
git rev-parse --show-toplevel
git rev-parse --git-dir
```

No index contents, `index.lock`, or post-failure repository state are claimed.

For run-3 D, A, and B, the diagnostic captured the exact `ToolError` described
above and the `StagedDiffExecution` stage. It captured no nonzero Git exit,
process-launch error, stderr, or output overflow. The `run` timeout guard is
before the next policy process call, so there was no failing child invocation
for which an exit code or stderr exists. The host Git path and fixed command
shapes are recorded above from source and the fixture helper; individual
successful probe timings and output were not recorded.

## Temporary diagnostic instrumentation

Temporary test-only `eprintln!` diagnostics were added only in the disposable
worktree's Desktop `main.rs`. They reported the staged `ToolError` and private
observation stage; run 3 used these to identify the timeout. Temporary logging
was also attempted in `rah-tools` sources behind `#[cfg(test)]`; as a normal
dependency of the Desktop test binary those crate-local `cfg(test)` blocks were
not compiled, so they supplied no process-level evidence. No Git command,
retry, sleep, assertion, timeout, lock, selector, repository refresh, or
production behavior was changed. All temporary edits were reverted and the
disposable worktree source files were verified against their committed blobs.

## Earliest-failure classification and symptom comparison

| Test | Earliest incorrect state in Task 414 | Classification |
|---|---|---|
| A | Staged `IndexVsHead` observation returns the shared-deadline error before the next fixed Git command. Refresh then supplies an empty status snapshot, so the first Stage action is absent. The test never reaches stale-selector resolution. | **1 — Git command execution boundary failed before spawn due the total timeout; no Git child returned nonzero.** |
| B | Direct snapshot collection returns `StagedDiffExecution` on the staged observation; run 3's underlying `ToolError` is the same total timeout. | **1 — Git command execution boundary failed before spawn due the total timeout; no Git child returned nonzero.** |
| C | Not reproduced in Task 414. Its direct indexing would fail if refresh received an observer error and published the empty fallback, but all eight Task 414 executions passed. | **7 — historical event remains unresolved; the reproduced timeout is mechanically sufficient but was not observed during C.** |
| D control | Same staged observation timeout leads refresh to publish an empty status vector, so the expected tracked modification has no Stage action. | **1 — Git command execution boundary failed before spawn due the total timeout; no Git child returned nonzero.** |

Answers to the cross-symptom questions:

- Yes. An observation error is sufficient to explain the missing Stage action:
  refresh discards the partial status/worktree observations and derives no
  actions from the empty fallback.
- Yes. An empty `staged_diff` is sufficient to explain C's index-out-of-bounds
  panic.
- A, B, and C all call the same staged-diff observation path. A and B
  reproduced the same total-timeout mechanism in run 3. C did not fail in Task
  414, so its historical cause is not established event-by-event.
- No independent selector/currentness rejection was observed. No second
  independent Git mechanism was isolated. Run 2's deletion/review failures
  were not diagnosed below the same staged-review boundary and are retained as
  unexplained run-2 evidence rather than forced into a separate root cause.

## Environment and validation boundaries

```text
Windows edition reported by Get-ComputerInfo: Windows 10 IoT Enterprise LTSC 2024
Windows version/build: 10.0.26100 / 26100
Architecture: x64 / 64-bit
rustc: 1.98.1 (48a229cea 2026-09-01)
Cargo: 1.98.1 (797e8a9bc 2026-08-05)
Git: 2.55.0.windows.5
Git executable: C:\Program Files\Git\cmd\git.exe
TEMP/TMP: F:\Temp
cargo clean: NOT USED
```

`git diff --check` passed for the main worktree's tracked diff before the
artifact was added. No antivirus, filesystem-filter, or indexing telemetry was
collected; there is no direct evidence for those causes. No native Git command
exit failure or stderr was captured at the timeout boundary.

Task 413 records the same build as Windows 11 IoT Enterprise LTSC. Task 414's
current `RuntimeInformation` and `Get-ComputerInfo` calls reported Windows
10.0.26100 and Windows 10 IoT Enterprise LTSC 2024. The edition-label
discrepancy is recorded, not reconciled.

## Task 412 disposition and next task

Task 412 remains:

```text
BLOCKED
```

Do not resume release preparation until a later correction and verification
task establishes stable workspace validation. Task 414 changed no Rust or test
source in the main worktree and committed no source or test code. No push or
tag occurred.

Recommended next task:

```text
Task 415 — Correct staged-diff aggregate timeout exhaustion while preserving
the fixed Git commands, repository identity checks, and selector/currentness
contracts.
```

Task 415 should independently audit how the staged-diff deadline covers its
repeated Git-layout probes and all five staged-observation phases, then stress
the affected tests before workspace validation. This is a recommendation only;
Task 415 was not started.
