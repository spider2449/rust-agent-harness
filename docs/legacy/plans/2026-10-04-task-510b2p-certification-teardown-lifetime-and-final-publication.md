# Task 510B2P — Certification teardown lifetime and final publication

Starting HEAD / local origin/master: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.

Plan: audit owners, apply lexical app/state scope, inspect incremental diff; run
preserved-snapshot smoke twice, focused Desktop and authoritative workspace Clippy,
typed/presentation/snapshot regressions, fmt/check/workspace/canonical Desktop and
complete frontend/static/Tauri/metadata/HostExplicit gates serially. Stop on required
failure. Only A authorizes publication, followed by natural exact-head CI. Do not
begin Task 510B or change admission/preferred baseline.

Cargo.lock task-start SHA256:
`9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe`.

## Owner audit and correction

Prior authoritative Clippy failed at certification_tests.rs:226:5 on
`clippy::drop_non_drop`, `drop(state)`. Exact type:
`tauri::State<'_, DesktopAppState>`. Local Tauri 2.11.5 state.rs defines a borrowed
`&T` wrapper; Clone copies the borrow; no Drop implementation or Arc field.
DesktopAppState directly owns Mutex<Persistence>, which directly owns
Option<rusqlite::Connection>, with no Arc or separate SQLite owner. Desktop state
also contains repository/runtime/composition Arc fields. The test-local runtime
Arc retains neutral runtime/conversation until the existing drop after awaited
disconnect; ConnectionState is already NotConnected. No separate local conversation
exists. The awaited disconnect State clone ends on return; no model turn is started.

Tauri App/AppHandle retain Arc<AppManager>; app teardown alone previously retained
the original SQLite connection (Task 510B2G/M). Keep the proven external inert
persistence replacement and original Persistence drop after runtime release.
The old state drop only ended the borrowed handle before app teardown; it did not
own or close SQLite. An ordinary block now ends state/app before the single
remove_dir_all outside it. No new production API, shutdown change, snapshot change,
frontend change, suppression, sleep, retry or ignored error.

## Validation and publication

Pending. Evidence directory: `F:/temp/task510b2p-evidence`.

## Executed results and mandatory stop

Final classification: **E — WORKSPACE / DESKTOP / FRONTEND GATE FAILED**.

Both target variables were `F:/temp/rah-task510b2g-target`. Source was frozen
through each build/test. Incremental before/after diff confirms only lexical
app/state scope and removal of explicit state/app drops. As the test is untracked,
`git diff -- certification_tests.rs` has no tracked baseline; the saved before-file
comparison supplies the exact incremental diff. No production source was edited.

| Gate | Current result | Evidence |
| --- | --- | --- |
| First corrected exact smoke | PASS: 1 passed / 0 failed / 0 ignored / 380 filtered, 89.61s | smoke1.log |
| Second corrected exact smoke | PASS: 1 passed / 0 failed / 0 ignored / 380 filtered, 91.33s | smoke2.log |
| Focused Desktop Clippy, all targets/features, -D warnings | PASS, exit 0; zero Rust/Clippy warnings; drop_non_drop gone | clippy-focused.log |
| Authoritative workspace Clippy, all targets/features, -D warnings | PASS, exit 0; no further masked lint | clippy-workspace.log |
| Typed certification + focused snapshot regressions | PASS: 5 passed / 0 failed / 103 filtered | typed-snapshot.log |
| Frontend presentation mapping | PASS: 1 passed / 0 failed / 380 filtered | presentation.log |
| cargo fmt --check | FAIL, exit 1, certification_support.rs:397 | fmt.log |
| Workspace check / workspace tests | Not run after required stop; no current counts; N's 1085/0/24 is historical |
| Canonical Windows Desktop | Not run after stop; no current canonical count; N's workspace-contained 358/0/20 is historical |
| Complete frontend/static, including remembered_workspace_layout_test.js | Not run after stop; no Edge timeout recurrence occurred in this task |
| Tauri 47/47/47/47/47 | Not run after stop; requirement remains unverified this task |
| Metadata 14 members/packages, all 0.33.0, edition 2024 | Not run after stop; no current executable closure claim |
| HostExplicit static + executable closure | Separate gate not run after stop; requirement exactly 11 remains unchanged in source |
| Cargo.lock | SHA256 9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe; unchanged from task start |
| git diff --check | PASS after stop |

Each corrected smoke used the preserved verified snapshot, real adapter,
neutral runtime and real Desktop composition, then awaited disconnect/shutdown.
Runtime became not alive, ConnectionState became NotConnected, runtime/conversation
ownership was released, original persistence was replaced and dropped, lexical
scope ended state/app, and a single remove_dir_all succeeded. No sleep/retry or
ignored cleanup error. Inference = 0; Tools = 0. Exact spawned paths were recorded;
app-server PIDs were 17444 and 7320 respectively.
Fixture roots removed:
`F:/Temp/rah-task510b2-4602141f-884a-45e8-92e6-272609d271ec` and
`F:/Temp/rah-task510b2-adbef9ec-3875-47ed-a0df-66713bbd2e92`.
Full pre/post ArtifactMeasurement equality passed twice, including after cleanup.
External inert replacement storage remains outside each fixture and is retained.

An extra unchanged-source smoke ran before the edit due to a failed local editor
command (Python unavailable). It passed 1/0/380 in 91.60s, PID 17520. It is saved
as pre-correction-extra-smoke.log and is not counted as either corrected smoke.
No source edit occurred while it ran. The edit was then applied with PowerShell.

Typed MissingArtifact, HashMismatch and VersionMismatch source chains passed in
the focused tests and real smokes. Snapshot source-hash measurement evidence,
exact-byte snapshot identity, complete reads, distinct source/snapshot file IDs,
and candidate path preservation passed focused snapshot tests. SnapshotMismatch
still carries boxed typed source_measurement and snapshot_measurement, with its
constructor preserved; no dedicated forced SnapshotMismatch execution was added
or claimed. Single-source-handle hash/copy and spawned exact snapshot paths were
unchanged. Public RuntimeFailure diagnostics and frontend JSON remain sanitized;
frontend regression returned only `codex_connection_failed`. Focused snapshot
redaction assertions exclude source/snapshot paths, full SHA and file ID.

## Exact formatting failure

`cargo fmt --check` found one diff in the O correction at
`crates/rah-runtime-codex/src/certification_support.rs:397`:

```diff
-                    | ArtifactMeasurementError::Unstable(_) => std::io::Error::other(
-                        "unstable certification measurement",
-                    ),
+                    | ArtifactMeasurementError::Unstable(_) => {
+                        std::io::Error::other("unstable certification measurement")
+                    }
```

This file was not edited by P. No diff was reported in the lexical-scope test.
Stopped at this gate without formatting correction, retry, downstream validation,
staging or publication. The task's scope permits only the teardown lifetime
correction; the existing nine O lint corrections remain preserved.

## Final snapshot measurement

Independent Get-FileHash/Get-Item/fsutil after stop matches the task-start and
both corrected smoke measurements. No snapshot recreation was necessary.
Path: `F:/Temp/rah-codex-certification-snapshot-18440-1791116997708427800-0/codex.exe`.
SHA256: `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.
File ID: `0x0000000000000000002e0000000a80a1`.
Length: **326872368**. Attributes: readonly/archive.
This is a final stop measurement; no pre-staging certification is claimed.

## Authority, ADR and publication disposition

Incremental P patch changes only certification-test teardown lifetime. No changes
to production admission/preferred baseline, ToolRegistry, active repository
authority, switching, leases, permissions, Trusted Profiles, mutation uncertainty,
remembered workspace semantics or provider/model authority. The complete tracked
WIP inspected remains certification seam support; original production admission
controls pass, current certified set remains only codex-cli 0.157.1 and preferred
baseline remains 0.157.1. Codex 0.160.0 remains **unadmitted / uncertified**.
No dependency/lock change by P. Full authority/publication gates remain withheld.

**ADR-B**: ADR 0005, ADR 0030 and Task 498 error envelope remain applicable;
no new ADR, neutral API or production teardown API.

Changed by P: certification_tests.rs, this plan, and a reference-only O note.
Historical O evidence/classification remains intact. All unpublished WIP and
reports are preserved. Committed file count **0**. No staging, commit SHA/message,
push or exact-head CI. HEAD equals local origin/master:
`436470337269546a1209d0b9803c2b8aa9e1f6b4`; no remote refresh occurred.
Worktree remains dirty, including untracked Task 510B2 reports. No clean-worktree
or publication claim. Task 510B remains incomplete and was not started.

Task 510B resume authorization: **WITHHELD**. Suggested next bounded task: correct
only the documented rustfmt layout and resume withheld gates, preserving these
successful teardown/Clippy results. A plus natural exact-head CI is still required.

Reference-only follow-up: [Task 510B2Q](2026-10-04-task-510b2q-rustfmt-recovery-and-certification-seam-publication.md) applies the documented rustfmt correction and resumes withheld closure gates. Historical P evidence and classification remain unchanged.
