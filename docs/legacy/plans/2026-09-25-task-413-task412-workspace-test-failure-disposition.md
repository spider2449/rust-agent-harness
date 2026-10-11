# Task 413 — Task 412 Workspace Test Failure Reproduction and Disposition

Date: 2026-09-25
Status: **STOP — TASK 412 WORKSPACE VALIDATION IS INTERMITTENT**
Task type: validation and research only

## Verdict

```text
STOP — TASK 412 WORKSPACE VALIDATION IS INTERMITTENT
```

The two Task 412-reported tests passed all three focused runs and the owning
Desktop package suite passed. The first full serial workspace run failed in
three Desktop tests, including one Task 412-reported test. Two of the three
workspace failures also reproduced in focused tests on clean Task 412 baseline
HEAD; the same tests passed in other invocations. No source or dependency
change explains this variability. The result is not a release-ready validation
disposition.

Task 412 must remain blocked. Do not resume release preparation until a
separately authorized task identifies and closes the intermittent validation
failure.

## Starting checkpoint and frozen Task 412 diff

Verified before running tests:

```text
HEAD: ad9e755ac6d973fb39c58ad88e905c682e6fe9ba
message: docs: record repo.list binary Windows certification
origin/master recorded by Task 412: f02b467525cdfbfae03b75e262f70ff24e79df25
starting worktree: DIRTY, as expected
```

The exact eight Task 412 release-preparation paths were:

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

The initial tracked-diff fingerprint from
`git diff --binary | git hash-object --stdin` was:

```text
ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3
```

There were no dirty Rust source or test files. The eight Task 412 files were
not edited by Task 413.

## Task 412 failure record and prior evidence

Task 412 reported that `cargo fmt --check`, `cargo check --workspace`, and
`cargo metadata --no-deps --format-version 1` passed. Metadata reported 13
packages, all version `0.32.0`, edition `2024`. Its
`cargo test --workspace -- --test-threads=1` run stopped after reporting:

```text
tests::observed_single_target_stage_and_unstage_refresh_and_consume_selectors
tests::old_repository_selector_cannot_resolve_to_new_repository_action
```

Task 412 did not capture complete counts or panic details. It stopped before
Clippy, commit, push, or publication.

Historical comparison evidence was read from the existing repository plans:

- Task 410 dispositioned four earlier Desktop failures after each focused
  test passed 3/3 and two consecutive full Desktop suites passed at 320 passed,
  0 failed, 18 ignored, 338 discovered. The original failures remain
  historical evidence; that result does not dispose the Task 412 failures.
- Task 411 pre-certification Desktop validation passed at 320 passed,
  0 failed, 18 ignored, 338 discovered. This is comparison evidence only.

## Test locations and behavior

Both Task 412 tests belong to package `rah-desktop`, test binary
`crates/rah-desktop/src/main.rs`, module `tests`, source file
`crates/rah-desktop/src/main_tests.rs`:

| Test | Source location | Behavior and direct fixture path |
| --- | --- | --- |
| `tests::observed_single_target_stage_and_unstage_refresh_and_consume_selectors` | `main_tests.rs:2837–2898` | Creates a fresh modified Git repository and `DesktopAppState`, selects the repository, observes one stage selector, stages once, checks selector consumption, refreshes, observes an unstage selector, unstages once, and refreshes to no staged changes. |
| `tests::old_repository_selector_cannot_resolve_to_new_repository_action` | `main_tests.rs:2899–2969` | Creates fresh modified repositories A and B, captures A's stage selector, switches to B, checks selectors differ, rejects A's stale selector without changing B, then consumes B's current stage selector. |

The tests share the observed-repository workflow and selector-currentness
mechanism. Both use `TestRepository::git_repository`,
`TestRepository::native_git`, a fresh `DesktopAppState`,
`replace_selected_repository`, and `refresh_repository_workflow`; both exercise
`repository_index_action` for Stage. The first additionally exercises
Unstage and action consumption after refresh. The second additionally
exercises repository replacement and stale-selector rejection.

Each test creates its own Git fixture(s) and storage fixture; they do not share
a repository or app state. The helper uses a timestamp and process-local
`NEXT_TEST_DIRECTORY` counter for temporary paths, sets Git identity in each
repository's local config, and removes the fixture on drop. The test bodies do
not change the process current directory or environment. The module's other
`OnceLock` state is limited to a fake executable path; no shared workflow
selector cache or repository singleton was found in these paths. Fixture Git
commands do inherit ambient Git configuration, so this inspection does not
rule out host Git configuration or another external influence. No specific
cross-test contaminator was established.

## Focused reproduction matrix

Each command was run separately from the unchanged current Task 412 worktree:

```text
cargo test -p rah-desktop tests::observed_single_target_stage_and_unstage_refresh_and_consume_selectors -- --exact --test-threads=1 --nocapture
cargo test -p rah-desktop tests::old_repository_selector_cannot_resolve_to_new_repository_action -- --exact --test-threads=1 --nocapture
```

| Test | Run 1 | Run 2 | Run 3 | Classification |
| --- | --- | --- | --- | --- |
| `observed_single_target_stage_and_unstage_refresh_and_consume_selectors` | PASS, 30.06s | PASS, 29.22s | PASS, 22.80s | Focused REPRODUCIBLY PASSING (3/3) |
| `old_repository_selector_cannot_resolve_to_new_repository_action` | PASS, 14.62s | PASS, 15.80s | PASS, 15.06s | Focused REPRODUCIBLY PASSING (3/3) |

All six invocations completed normally, with one test passed and 337 filtered
out each. They emitted no panic or assertion output. These focused outcomes do
not override the later suite failure.

## Owning-package suite

Exact command:

```text
cargo test -p rah-desktop -- --test-threads=1
```

Result: **PASS**, 320 passed, 0 failed, 18 ignored, 338 discovered; test-harness
duration 843.24 seconds. Both Task 412 test names passed in their normal suite
positions. Git printed LF-to-CRLF warnings for temporary fixture files; no
failure was associated with those warnings.

## Full workspace run 1

Exact command:

```text
cargo test --workspace -- --test-threads=1
```

Result: **FAIL**, Desktop test binary reported 317 passed, 3 failed, 18
ignored, 338 discovered; duration 897.36 seconds. The preceding `rah-cli`
integration binary passed 14 tests. The full output was retained temporarily
outside the repository during triage. Complete Desktop failure evidence:

```text
---- tests::old_repository_selector_cannot_resolve_to_new_repository_action stdout ----
thread 'tests::old_repository_selector_cannot_resolve_to_new_repository_action' panicked at crates\rah-desktop\src\main_tests.rs:2929:10:
B exposes one stage action

---- tests::repository_snapshot_matrix_isolated_repositories_and_replacements stdout ----
thread 'tests::repository_snapshot_matrix_isolated_repositories_and_replacements' panicked at crates\rah-desktop\src\main_tests.rs:2194:10:
A snapshot succeeds: StagedDiffExecution

---- tests::review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors stdout ----
thread 'tests::review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors' panicked at crates\rah-desktop\src\main_tests.rs:2999:43:
index out of bounds: the len is 0 but the index is 0

test result: FAILED. 317 passed; 3 failed; 18 ignored; 0 measured; 0 filtered out; finished in 897.36s
```

The Task 412 stage/unstage test passed in this run. The old-selector test
failed while refreshing B because no stage action was exposed. Two other
repository-observation/review tests also failed. The failures did not have a
stable set across invocation contexts: current focused and package runs passed
the original targets, while the workspace run failed one of them.

Workspace run 2 was **not run**. Run 1 was not clean, and Task 413 stopped
workspace repetition to research the failure set first. Therefore the
two-consecutive-workspace-pass non-reproduction criterion is not met.

## Failure-set research and historical comparison

The current worktree's two additional workspace failures were checked
individually. `repository_snapshot_matrix_isolated_repositories_and_replacements`
failed focused at `main_tests.rs:2228` with:

```text
assertion failed: desktop_repository_snapshot(&first_a).await.is_ok()
```

`review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors`
passed focused at 1 passed, 0 failed, finished in 18.96s. Thus the additional
failures were not a repeatable workspace-only set either.

A clean temporary worktree at `ad9e755ac6d973fb39c58ad88e905c682e6fe9ba`
was created outside the dirty Task 412 worktree. It was clean and detached at
the requested HEAD. Focused baseline results:

| Test | Clean baseline result |
| --- | --- |
| `old_repository_selector_cannot_resolve_to_new_repository_action` | PASS, 1 passed, 337 filtered, 24.56s |
| `repository_snapshot_matrix_isolated_repositories_and_replacements` | FAIL at `main_tests.rs:2194`, `A snapshot succeeds: StagedDiffExecution`; 0 passed, 1 failed, 16.22s |
| `review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors` | FAIL at `main_tests.rs:2985`, index out of bounds (`len is 0`, index 0); 0 passed, 1 failed, 12.13s |

The temporary baseline worktree was removed after confirming it was clean.
These results establish that the latter two failure modes exist at the clean
0.31.0 checkpoint under the same host environment. They do not establish why
the tests pass in some invocations and fail in others. No smallest preceding
test family or concrete shared-state leak was identified. No second workspace
run was started.

## Version, source, and dependency assessment

`git diff -- '*.rs'` was empty at start and after validation. No Rust source or
test source was changed. The Task 412 Cargo diff changes the workspace version
`0.31.0` to `0.32.0`; the lockfile changes only the 13 internal RAH package
version records. The dependency lists and external package versions/checksums
are unchanged. There is no feature or dependency drift in the Task 412 diff.

The clean baseline reproduces two of the workspace failure modes, and the
current 0.32.0 focused/package results pass the two Task 412-reported tests.
The workspace package version is compiled into the Desktop `app_version`
presentation through `CARGO_PKG_VERSION`; the failing selector and staged
repository assertions do not inspect that presentation. No evidence
establishes version or release-metadata causality. The intermittent workspace
result remains unexplained and is not waived by this assessment.

## Validation environment and procedure limits

```text
Windows edition: Microsoft Windows 11 IoT Enterprise LTSC
Windows version: 10.0.26100
Windows build: 26100
Architecture: x64-based PC / 64-bit
rustc: 1.98.1 (48a229cea 2026-09-01)
Cargo: 1.98.1 (797e8a9bc 2026-08-05)
Git: 2.55.0.windows.5
cargo clean: NOT USED
```

The Task 412 release-preparation diff remained frozen. Task 413 changed no
production code, tests, release metadata, release documentation, tag, or remote
state. No release preparation was resumed. No push or tag occurred.

## Final disposition and next task

```text
STOP — TASK 412 WORKSPACE VALIDATION IS INTERMITTENT
Task 412 resume: NO
```

The original two Task 412 failures are not reproducible in focused runs or the
owning-package suite, but one reappeared in workspace context. The broader
workspace result additionally contains intermittent failures present on the
pre-release baseline. This is not Outcome A: there were not two consecutive
green workspace suites, and intermittency remains observed.

Recommended next task:

```text
Task 414 — Reproduce and Isolate Intermittent Desktop Git Observation Test Failures
```

Task 414 should preserve the Task 412 diff, investigate the native Git
observation/staged-diff failure modes under repeated focused, Desktop-package,
and carefully controlled workspace execution, and identify the environmental
or test-isolation cause before any authorized correction. Task 414 should not
resume release preparation unless the required validation disposition is
established. No Task 414 work is started here.

## Task 413 artifact and closeout

```text
Artifact: docs/plans/2026-09-25-task-413-task412-workspace-test-failure-disposition.md
Rust/test source changes: none
Task 412 release-preparation files: unchanged and still uncommitted
starting and ending tracked-diff fingerprint: ff2f33adcd3c7a38ad4d43cdbfe1f198cbbe6fa3
Task 413 commit message, if committed: docs: disposition v0.32 release validation failures
Push/tag: none
```

Before the documentation-only commit, `git diff --check` and the exact staged
path set must be verified. Only this Task 413 artifact may be committed.
