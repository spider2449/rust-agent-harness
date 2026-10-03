# Task 504E — Desktop neutral migration compile corrections

Starting and final HEAD: `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077`.
Task 504 migration WIP remains dirty and uncommitted. No Task 504 commit or CI exists.

## Audit and narrow corrections

Task 504D recorded E0308 at main.rs:8513 and main_tests.rs:6316/6812
(OsString supplied to a PathBuf parameter), and E0599 at main_tests.rs:6498
(neutral TurnHandle has no into_events method).

All three path values are `PreparedCodexConnection.executable`, populated from
the host executable resolver. The certified baseline is verified as a filesystem
PathBuf and returned as OsString; host override and PATH executable selection
also retain native OS strings. These are executable paths, not repository fixtures
or model configuration. `configured_codex_factory` passes its PathBuf into
`CodexFactory::new`; its path contract is appropriate.

At exactly those three callers, `prepared.executable` became
`PathBuf::from(prepared.executable)`. This owned conversion preserves native
path contents, including Windows non-Unicode OS strings, without String conversion,
normalization, re-resolution or changes to executable certification. Both live
test fixtures now use the same conversion as production.

At main_tests.rs:6498, `let mut events = handle.into_events();` became
`let mut events = handle.events;`. The neutral TurnHandle owns public session_id,
events and separate shareable control fields. Moving events follows its intended
single-owner stream API and the existing production consumer at main.rs:9350.
DesktopRuntime::start retains a clone of control separately. No neutral API was
added; cancellation ownership and test assertions were not changed.

## Phase B — compilation failure; stop

Environment: CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR both
`F:/temp/rah-task504-target`, with Task 504D's prepared provider fixtures.
The exact previously failing command was executed once:

```powershell
cargo test -p rah-desktop --bin rah-desktop -- task504 model_preflight::tests:: desktop_model_selection_maps_only_closed_provider_choices connected_repository_selection_rejects_direct_activation_without_changing_authority local_runtime_failures_project_closed_frontend_codes activation_publication_requires_repository_model_profile_and_connection_currentness
```

Exit **101**. No tests executed; passed/failed/ignored counts are **unavailable**.
Complete Cargo log: `F:/temp/rah-task504e-evidence/phase-b.log`.
Exit evidence: `F:/temp/rah-task504e-evidence/phase-b.exit`.

The original four diagnostics no longer appear. Compilation exposed **11 E0308
diagnostics**: the stream yields RuntimeEvent while the existing consumer expects
AgentEvent. One is the activity_event_with_composition argument at line 6502;
ten are AgentEvent match patterns at lines 6511, 6516, 6531, 6545, 6549, 6553,
6557, 6558, 6559 and 6560. This is a newly exposed test-consumer compile blocker,
not evidence that the neutral ownership contract or executable path API is wrong.
Stopped without correction, retry or further validation.

## Preserved and remaining evidence

Task 504D Phase A remains **137 passed, 0 failed, 1 ignored**, with both doctest
groups containing zero tests. None of the three historical bridge failures recurred;
Task 503 adapter conformance coverage passed in that run. Phase A was not rerun.
All five runtime/adapter source SHA256 values and ADR 0033 match Task 504D's
recorded freeze, verified after Phase B exited.

Canonical Desktop counts, workspace counts, fmt/check/Clippy, frontend/static,
Tauri inventory, metadata, executable HostExplicit, Task 499 regression and
additional Task 503 regression: **NOT RUN**, because Phase B did not compile.
Production Connect/turn/Tool/Disconnect, absent-model and repository-switch
validation: **NOT RUN**. No resumed gate or production certification is claimed.

Static HostExplicit inventory remains **11**: FsRead, RepoFileInfo, RepoStatus,
RepoDiff, RepoDiffStaged, RepoCreateBranch, RepoPatch, RepoEditFiles,
RepoCreateFile, RepoDeleteFile, RepoRenameFile. Executable certification is pending.
**ADR 0033 unchanged**. No dependency, API, ToolRegistry authorization,
repository authority/switching, lease, permission, Trusted Profile, mutation
uncertainty, remembered-workspace or model-preflight policy change was introduced.
Those runtime invariants remain unverified on the migrated Desktop.

## Closure

Only main.rs, main_tests.rs, this report and a reference-only Task 504D append
were changed by Task 504E. Existing migration WIP is preserved.
Closure git status --short, git diff --stat and git diff --check executed;
diff check exit 0 (line-ending warnings only).
No commit, staging, push, CI, tag, release or version bump. Final worktree remains
dirty, not clean. A later narrow task should audit the RuntimeEvent consumer
against the existing Desktop event projection, then rerun Phase B. No
alternate-provider work started.

**E — PHASE B STILL FAILS TO COMPILE**

## Task 504F reference

[Task 504F](2026-10-03-task-504f-runtimeevent-agentevent-boundary-correction.md) records the event audit, eleven E3 sites redirected to the existing borrowed projection, passing Phase B and completed validation. This reference preserves Task 504E's historical compilation stop evidence.
