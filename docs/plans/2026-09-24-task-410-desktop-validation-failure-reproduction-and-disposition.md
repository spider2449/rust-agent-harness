# Task 410 — Desktop Validation Failure Reproduction and Disposition

## Scope and starting checkpoint

Task 410 independently reproduces and dispositions the four Desktop tests that
failed during Task 409's required package validation. This is validation and
research evidence only. No Rust source, tests, ignored markers, authority
semantics, `repo.list` behavior, certification harness, release preparation,
or Task 411 work was changed or started.

Starting checkpoint verification:

```text
HEAD: f02b467525cdfbfae03b75e262f70ff24e79df25
git status --short: empty (clean)
```

Task 409 stopped before Windows live certification after the Desktop package
suite reported `316 passed; 4 failed; 18 ignored; 0 measured`. Its deterministic
binary-file evidence gate remains closed at commit
`580880f2619de4ed1e3810aec31b2ad27d6d91c3`:

```text
DETERMINISTIC REPO.LIST BINARY EVIDENCE: CLOSED
```

No Task 409 certification step was resumed here.

Task 409's recorded failure points and observed assertions are preserved
below. These are the line locations from the Task 409 validation run, before
this independent reproduction:

| Test | Task 409 source location | Observed failure |
| --- | --- | --- |
| `task349_close_clears_pending_no_effect_commit_authorization` | `main_tests.rs:9542` | `staged repository should have an authorizable review` |
| `task_321_c_authorization_preparation_cannot_rearm_after_activation` | `main_tests.rs:10887` | `fresh review observes: StagedDiffExecution` |
| `task_321_e_unstage_reservation_wins_over_activation` | `main_tests.rs:11385` | `A should expose an Unstage action` |
| `task_321_i_real_stage_reservation_rejects_real_connect_publication` | `main_tests.rs:9047` | `workflow should expose the requested real index action` |

## Desktop source and dependency comparison

The last known passing Desktop checkpoint is Task 406 commit
`94c78613f923921b492a776ce558cecd00d37b81` (`test: cover effective authority
expiry bookkeeping`). Task 406 recorded `320 passed; 18 ignored; 0 failed; 338
discovered`.

Desktop tree object comparison:

```text
94c78613f923921b492a776ce558cecd00d37b81:crates/rah-desktop
d0701b0884564372d7c3c44583e5536f1cc1550c

HEAD:crates/rah-desktop
d0701b0884564372d7c3c44583e5536f1cc1550c
```

The Desktop tree is byte-identical. The comparison from Task 406 to Task 410
found no changes in `crates/rah-desktop`, workspace `Cargo.toml`, or
`Cargo.lock`. The only non-documentation source delta in the range is the 82
line addition to `crates/rah-tools/src/repository_list.rs` at the Task 409
evidence commit. It adds the binary no-content test inside the existing
`#[cfg(test)]` test module. It does not change the `rah-tools` production
library, manifests, features, or lockfile. Thus it cannot change the
production dependency artifact consumed by `rah-desktop`.

There is no source or dependency delta connecting Task 409's binary evidence
addition to the four Desktop failures. Chronology alone is not treated as
causal evidence.

## Four-test source and workflow inventory

All four tests are in `crates/rah-desktop/src/main_tests.rs`.

| Test | Source range | Workflow and expected state | Fixture and shared helpers |
| --- | --- | --- | --- |
| `tests::task349_close_clears_pending_no_effect_commit_authorization` | 10742–10774 | On staged repository A, capture a staged review, prepare and install Commit authorization without dispatch, then Close must clear pending authorization, capability, and workflow state and advance repository generation once. | `close_activation_fixture(GitRepositoryState::Staged)`, `install_close_commit_review`; each builds its own `DesktopAppState`, storage fixture, and Git repositories. |
| `tests::task_321_c_authorization_preparation_cannot_rearm_after_activation` | 10864–10940 | Pause authorization after async preparation for A, activate B, then resume; A's stale writer must not restore pending authorization or overwrite B's `ReadyToAuthorize` presentation. | Two fresh Git repositories (A staged, B clean), one `DesktopAppState`, `authorize_test_commit`, `desktop_repository_snapshot_with_review`, and per-state authorization barrier installed by `install_authorization_barrier`. |
| `tests::task_321_e_unstage_reservation_wins_over_activation` | 11362–11415 | Reserve a real Unstage effect for A and pause before effect; activation of B must return `RepositoryBusy`; after Unstage completes and clears the reservation, activation of B must succeed. | Two fresh Git repositories (A staged, B clean), one `DesktopAppState`, `refresh_repository_workflow`, `repository_index_action`, and per-state index-effect barrier from `install_index_effect_barrier`. |
| `tests::task_321_i_real_stage_reservation_rejects_real_connect_publication` | 9304–9447 | Complete an initial real Stage reservation, begin Connect, then install another real Stage reservation before provider publication. Publication must reject with `IndexEffectActive`, preserve the reservation, avoid publishing provider/Commit state, and return the connection to `NotConnected`. | `activate_real_index_fixture(Stage)`, `complete_repository_index_effect`, `desktop_tool_registry`, `test_codex_runtime`, `start_real_index_reservation`; all operate on the test's fresh state and repositories. |

The tests exercise separate paths across Commit review/authorization, Close,
repository activation, Stage/Unstage index reservations, and connection
publication. Their common dependencies are native Git fixtures and the
Desktop's lifecycle/workflow machinery; they do not share a repository fixture
or `DesktopAppState`.

## Shared-state and fixture contamination review

Inspection of the test module and these helpers found:

- Test repositories are created under `std::env::temp_dir()` at unique
  timestamp plus process-local atomic sequence paths. `TestRepository::drop`
  removes the owned path. Git-backed fixtures initialize isolated repositories
  and set `user.name` and `user.email` in each repository's local config.
- The helper locates native Git using `where.exe`; tests also use the current
  test executable for test-only Desktop repository construction. These are
  environmental dependencies, not shared fixture paths.
- The test module has `NEXT_TEST_DIRECTORY: AtomicU64` and a
  `FAKE_CODEX_EXECUTABLE: OnceLock<PathBuf>`. They allocate unique fixture
  locations and cache a fake executable path; neither stores repository
  workflow or authority state.
- Authorization, activation, connect-publication, and index-effect test hooks
  are installed on each `DesktopAppState`. Their barriers/channels are local
  to that state. The four tests do not use a process-global synchronization
  hook or singleton registry.
- No `set_var`/`remove_var`, fixed repository cache, shared generation, or
  shared runtime state is used by the four tests or their direct fixture
  helpers. Environment reads in those helpers are limited to temp directory,
  current executable, and native Git discovery.
- Local Git identity is fixed per fixture. Windows Git emitted LF-to-CRLF
  warnings during unrelated tests in both full-suite runs, showing that Git's
  configured line-ending behavior can affect diagnostics; the warnings did
  not correspond to any failed test. No evidence tied that configuration to
  the Task 409 failures.

This review found no shared-state contamination mechanism among the four
tests. It does not prove that every host-level environmental influence is
absent.

## Focused exact reproductions

Each command used the exact test identity, `--exact --test-threads=1
--nocapture`, at the unchanged starting HEAD. No source was changed between
runs. Every invocation ran one test with 337 filtered out (338 discovered).
Runtime below is the test harness's reported duration; the first invocation
also incurred the initial test-binary compilation and Cargo build-lock wait.

| Test | Run 1 | Run 2 | Run 3 | Classification |
| --- | --- | --- | --- | --- |
| `task349_close_clears_pending_no_effect_commit_authorization` | PASS, 32.54s | PASS, 27.16s | PASS, 23.35s | REPRODUCIBLY PASSING |
| `task_321_c_authorization_preparation_cannot_rearm_after_activation` | PASS, 28.49s | PASS, 28.90s | PASS, 27.49s | REPRODUCIBLY PASSING |
| `task_321_e_unstage_reservation_wins_over_activation` | PASS, 13.40s | PASS, 14.73s | PASS, 12.02s | REPRODUCIBLY PASSING |
| `task_321_i_real_stage_reservation_rejects_real_connect_publication` | PASS, 18.57s | PASS, 17.20s | PASS, 15.47s | REPRODUCIBLY PASSING |

All 12 invocations reached their normal final assertions. There was no
failure point or unexpected state in focused reproduction, and no
intermittent result was observed.

## Full Desktop suite reproduction

The exact Task 409 package command was run twice consecutively without source
changes:

| Run | Command | Passed | Failed | Ignored | Discovered | Duration |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | `cargo test -p rah-desktop -- --test-threads=1` | 320 | 0 | 18 | 338 | 838.35s |
| 2 | `cargo test -p rah-desktop -- --test-threads=1` | 320 | 0 | 18 | 338 | 799.40s |

Both runs passed. Each named failure passed when encountered in the full
suite's alphabetical test order. The failing-test set did not change between
the two runs: neither run had a failing test. Together with 3/3 focused passes
for each case, this meets the Task 410 non-reproduction threshold.

No historical worktree comparison was required because no failure reproduced
at current HEAD. No `cargo clean` was run. During the first focused command,
Cargo waited on a build-directory lock while Rust Analyzer had a separate
workspace check active; compilation completed and the requested command
passed. The unrelated check was left untouched. Later runs completed without
that initial compilation delay.

## Validation environment

Captured for this Task 410 validation:

```text
Windows: Microsoft Windows 11 IoT Enterprise LTSC
Version: 10.0.26100
Build: 26100
Architecture: x64-based PC
rustc: 1.98.1 (48a229cea 2026-09-01)
cargo: 1.98.1 (797e8a9bc 2026-08-05)
Git: 2.55.0.windows.5
```

This is validation-triage evidence, not a certification-environment claim.
Task 382 records the same Windows edition/version/build/architecture and the
same rustc/Cargo/Git versions. Task 406 recorded its passing Desktop suite but
did not record an environment block, so exact Task 406 environment identity
cannot be established from that artifact.

## Causal assessment and disposition

The original Task 409 validation failure did occur and remains part of the
record: four Desktop tests failed in that run, so Task 409 correctly stopped
before Windows live certification. This Task 410 reproduction found:

- byte-identical Desktop source at Task 406 and current HEAD;
- no workspace or Desktop Cargo manifest/lockfile delta;
- only a `#[cfg(test)]` test addition in `rah-tools` since Task 406;
- all four tests passing 3/3 individually;
- two consecutive full Desktop suite passes with identical 320/0/18/338
  counts; and
- no shared process-global workflow state or fixture leak in the inspected
  test paths.

The specific trigger for the original Task 409 failure was not identified.
There is no reproduced Desktop code defect and no evidence that the
`rah-tools` deterministic test caused it. The original event is therefore
dispositioned as a **transient, non-reproduced validation failure**. This does
not establish that it never happened or explain its exact trigger.

## Verdict and next task

```text
PASS — TASK 409 DESKTOP VALIDATION FAILURES DISPOSITIONED AS NON-REPRODUCIBLE
```

Task 409 may resume in a separately authorized next task. Its Windows live
binary certification gate remains unrun and open. No certification claim is
made here.

Recommended next task, not started:

```text
Task 411 — Resume repo.list Binary-File Windows Live Certification
```

Task 411 should start from this Task 410 clean commit, restore or recreate only
the prepared binary live-harness case, validate and commit that harness before
certification, record the exact certification HEAD, run the real Windows
active Desktop registry `repo.list` binary case, and prove the structural
entry plus absence of contents, sentinel, and privacy/isolation leaks. It
should close Task 409's remaining Windows gate without repeating deterministic
binary implementation or starting release preparation.

## Files, validation, and Git closeout

Expected Task 410 file scope: this document only. Rust source changes: none.
No dependency, ADR, authority, ignored-marker, version, release, or
certification-harness change was made.

Validation required for this artifact:

```text
git diff --check: PASS
git status --short before commit: this document only
```

Commit only this document as:

```text
docs: disposition desktop validation failures
```

After commit, verify clean status, record commit SHA and summary, and confirm
that no push or tag occurred.
