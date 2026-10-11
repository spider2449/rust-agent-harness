# Task 510B2G — certification cleanup lock-owner diagnosis

Status: diagnosed — H2

This plan records the bounded diagnosis of the Windows error 32 observed by
Task 510B2F after the real certification smoke returned from shutdown. It does
not authorize a production lifecycle correction, cleanup retry as a fix, model
certification, admission change, or broader validation gates.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`

## Scope

- Record the exact cleanup target and operation at `certification_tests.rs`.
- Map visible local and process lifetime owners.
- Capture Codex process identity before shutdown and immediately after it.
- Record bounded cleanup timing as diagnostic evidence only.
- Classify the owner as H1–H6 before recommending any correction.

## Constraints

- Preserve the Task 510B2F manifest/linkage correction and typed/presentation
  evidence.
- Keep Codex 0.160.0 uncertified and preserve the current admission/preferred
  baseline.
- Do not add retry/sleep, forced kill, cleanup suppression, or explicit drops as
  a behavioral correction before ownership is proven.
- Do not run full workspace, canonical Desktop, Tauri, or HostExplicit gates
  before classification.

## Evidence record

To be completed from the focused smoke diagnostics:

1. exact cleanup target, operation, creator, and possible owners;
2. process snapshots before and after shutdown;
3. cleanup timing and Windows error details;
4. handle-tool evidence or controlled elimination;
5. final H1/H2/H3/H4/H5/H6 classification;
6. production lifecycle implication and narrow next correction scope.

## Completed evidence

The cleanup statement is `std::fs::remove_dir_all(root).unwrap()` at
`crates/rah-desktop/src/certification_tests.rs:214` in the pre-diagnostic
source. `root` is created by the test with
`temp_dir()/rah-task510b2-<SessionId>`, passed to `DesktopAppState::new`, and
contains `conversation-transcript.sqlite3` after the real Desktop composition
starts. The operation is recursive directory removal; it does not directly
target the Codex executable.

The visible owners were the Tauri `App` and managed `DesktopAppState`, its
`Persistence` mutex containing a rusqlite `Connection`, the connection/runtime
state, the Codex `ProcessTransport` child and stderr task, the app-server
connection task, and the temporary root. No candidate executable copy or log
file was created beneath this root.

Two diagnostic smoke runs reached the real adapter, neutral runtime, Desktop
composition, shutdown, and the existing `inference=0; Tool execution=0`
assertion. In the first diagnostic run the candidate app-server was PID 18708,
parent PID 17540; it existed before shutdown and was absent immediately after
`disconnect_codex` returned. In the second it was PID 14408, parent PID 17660;
the same before/after result held. The only remaining `codex.exe` was the
pre-existing external Codex PID 3848, parent PID 3928 (`node.exe`), with a
different no-app-server command line. It was present before and after both
runs and is unrelated to the test candidate. No `handle.exe`, Process
Explorer, or equivalent lock-owner utility was available.

The first diagnostic run attempted removal at approximately 1, 52, 303, and
1305 ms after `drop(app)`; each returned Windows error 32. The second run used
the same bounded timing and found the local persistence connection present.
The test-only probe removed that exact `Persistence` SQLite `Connection` and
then `remove_dir_all(root)` succeeded at 17 ms. An independent exclusive open
of the SQLite file succeeded after the failed test process exited. This is
specific local-owner evidence, not timing-only inference.

The required repeat attempt did not reach cleanup: the installed candidate
changed from the recorded hash `fdda5fa3...e866d1d` to
`100bc9ee...10ed890`, so the pre-launch identity assertion failed. The expected
hash and admission/preferred baseline were left unchanged.

## Classification and consequence

**B — H2: RAH/TEST PROCESS RETAINS A FILE OR ASYNC HANDLE.** The retained
resource is the RAH Desktop `Persistence` rusqlite `Connection` for
`conversation-transcript.sqlite3`, held by the managed `DesktopAppState` when
the test attempts to remove its storage root. The unrelated external Codex
process is not implicated, and the candidate process is gone after shutdown.

Production lifecycle is not implicated by the process evidence: the owned
candidate process exited before cleanup. The narrow next correction scope is
the Desktop/test storage lifetime boundary: release the RAH-owned persistence
connection (or otherwise ensure its owning state is dropped) before removing
the temporary root. Do not add cleanup retry/sleep, change Codex process
reaping, alter admission, or certify 0.160.0 based on this evidence.

The diagnostic accessor, process snapshots, and bounded timing loop were
removed after evidence capture. No production behavior correction was made.

## Result

No production source correction or commit was made by this diagnosis. ADR 0005
continues to require the Codex app-server process boundary; ADR 0030 continues
to govern exact version admission. No authority, ToolRegistry, repository,
admission, or preferred-baseline boundary changed. Full workspace, canonical
Desktop, Tauri, and HostExplicit gates were not run.

Task 510B2H follow-up is recorded in
[Task 510B2H](2026-10-04-task-510b2h-certification-test-persistence-teardown-and-seam-closure.md).
