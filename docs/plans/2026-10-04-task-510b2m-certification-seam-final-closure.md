# Task 510B2M - Certification seam final closure attempt

Reference-only follow-up: [Task 510B2N](2026-10-04-task-510b2n-remembered-workspace-edge-timeout-and-seam-publication-closure.md)
classified the Edge timeout H1 after two unchanged isolated PASS executions;
resumed workspace tests passed, then clippy failed under D. Historical M
classification and evidence below remain unchanged; publication remains withheld.

Final classification: **F - FULL DETERMINISTIC / DESKTOP / AUTHORITY GATE FAILED**.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Task 510B2 remains unpublished WIP. No staging, commit, push or exact-head CI.
Task 510B must not resume. Codex 0.160.0 remains unadmitted / uncertified;
preferred/certified 0.157.1 and its baseline artifact were not modified.

## Plan and stop policy

Audit current ownership, revalidate the preserved snapshot, apply a narrow
test-only teardown correction, run the smoke twice, run focused regressions,
then full deterministic/Desktop/static/authority gates. Only complete A would
authorize staging, commit, normal GitHub master push and natural exact-head CI.
Stop on a required gate failure; do not retry cleanup or failing validation.
Preserve all previous classifications and reports historically.

## Verified snapshot

Preserved artifact:
`F:/Temp/rah-codex-certification-snapshot-18440-1791116997708427800-0/codex.exe`.

- SHA256: `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.
- File ID: `0x0000000000000000002e0000000a80a1` (12947848929378465).
- Bytes read, length before and length after: each **326872368**.
- Volume serial: 2114581433; attributes: 33 (readonly/archive).
- Creation FILETIME: 134355905977089664.
- Last-write FILETIME: 134355906084729114.

Before the first smoke, independent PowerShell Get-FileHash and fsutil queryfileid
matched the preserved SHA and file ID. Both smokes used measure_artifact's
complete stable opened-handle read before controls, immediately before composition,
after disconnect and after cleanup. Full ArtifactMeasurement equality passed.
No preserved snapshot recreation was necessary; the artifact is retained.

All certification executable launches used verified snapshot paths. The live npm
path was used only by the existing normal production admission rejection control.
No PATH fallback. Single-handle snapshot creation/hash/copy semantics unchanged.

## Ownership audit and narrow correction

DesktopAppState owns Mutex<Persistence>; Persistence directly owns
Option<rusqlite::Connection>, with no Arc, Clone or separate database owner.
The Tauri app's manager owns DesktopAppState. Local State and its disconnect
clone borrow the managed state; they do not clone DesktopAppState or persistence.
The clone passed into awaited disconnect ends when that call returns.

The injected factory is moved into the awaited composition call and dropped when
it completes. Composition binds the real neutral runtime/conversation into
DesktopRuntime, retained by ConnectionState and one test runtime Arc. No app
handle is supplied to this composition; no chat/turn task is started. Disconnect
revokes and awaits shutdown, then withdraws the connection to NotConnected.
The runtime clone was previously retained through persistence cleanup. This task
explicitly drops it before releasing the persistence owner, releasing its neutral
instance/conversation owners. No other test-local conversation or runtime owner.

Retained the corrected Task 510B2H external persistence replacement: replace the
managed persistence with an instance rooted outside the fixture, then drop the
original value/SQLite connection, state borrow and app, then remove the fixture
once. Tauri State is a borrow; app dropping alone was previously insufficient.
Tauri's managed-state removal API is deprecated and documented unsafe. An
ordinary app scope does not prove manager destruction; this test keeps the narrow
existing ownership replacement rather than adding production APIs or unsafe
state removal. The replacement storage is retained outside the fixture.

Teardown order now proven by successful cleanup: Desktop disconnect/shutdown;
runtime not alive and connection NotConnected; drop retained runtime/conversation;
replace/drop original persistence (SQLite connection); drop state/app; one
remove_dir_all succeeds. No sleep/retry, ignored error, production persistence
edit, runtime shutdown edit, snapshot algorithm edit or new production API.

Pre-validation diff against the saved test source was inspected. Changes were
confined to certification test execution/measurement and lifetime/cleanup evidence.
Original source: `F:/temp/task510b2m-evidence/certification_tests.before.rs`.
Source remained frozen during builds/tests.

## Executed focused gates

Both target variables: `F:/temp/rah-task510b2g-target`.
Evidence directory: `F:/temp/task510b2m-evidence/`.

| Gate | Result | Evidence |
| --- | --- | --- |
| First exact snapshot Desktop smoke | 1 passed / 0 failed / 0 ignored / 380 filtered; 106.05s | smoke1.log |
| Second same-snapshot smoke | 1 passed / 0 failed / 0 ignored / 380 filtered; 100.80s | smoke2.log |
| Presentation mapping | 1 passed / 0 failed / 380 filtered | presentation.log |
| Typed verification and focused snapshot suite | 5 passed / 0 failed / 0 ignored / 103 filtered | focused.log |
| Snapshot campaign control | 1 passed / 0 failed / 0 ignored / 380 filtered; 147.23s | campaign.log |
| Direct executable loader/list | exit 0; 381 tests | loader.log |
| Embedded Common Controls v6 manifest | extraction exit 0; v6 dependency present | certification.manifest, manifest.log |
| cargo fmt --check | PASS, exit 0 | fmt.log |
| cargo check --workspace (already running at stop) | PASS, exit 0; 2m 27s | check.log |
| git diff --check | PASS | tool output |

Both smokes used the real Codex adapter/app-server, neutral runtime, and Desktop
production composition with bounded RuntimeDefault pre-turn connect/disconnect.
Chat Idle, runtime alive then not alive, Connected then NotConnected; inference
**0**, Tool execution **0**. Exact spawned snapshot path was printed at spawn.
PIDs: first **16824**, second **18096**. Post-disconnect process inspections showed
neither PID; only unrelated pre-existing Codex PID 3848 remained. Final process
inspection found none of the four campaign/smoke PIDs or their immediate children.

Cleanup succeeded without retry for both fixture roots:
`F:/Temp/rah-task510b2-9d715fc1-827f-4d12-b556-7506a8cbdac6` and
`F:/Temp/rah-task510b2-9880e10d-207f-481d-bb41-98de94f26ad7`.
Full pre/post snapshot identity matched in each smoke, including SHA/file ID/length.

Each smoke passed exact supplied missing path, intentionally wrong SHA, wrong
version, and production rejection controls. Typed MissingArtifact preserves the
supplied missing path; HashMismatch preserves zeros versus the exact SHA;
VersionMismatch preserves 0.157.1 versus `codex-cli 0.160.0`. Rejections occur
before app-server startup (wrong version performs version probing).
Normal production still rejects installed 0.160.0 as VersionMismatch before
app-server startup. Production admission remains separate from certification.

CertificationVerification maps to FrontendError::CodexConnectionFailed and JSON
`"codex_connection_failed"`; RuntimeFailure diagnostics remain sanitized while
the real process-local source chain recovers exact typed values. Measurement
records remain process-local and are not serialized to frontend.

Five focused tests include complete stable measurement, snapshot bytes/identity
and rejections, exact descriptor rejection, version/production separation, and
wrong hash cannot probe/start. The existing campaign creates a fresh snapshot via
the validated procedure, independently rehashes it (including PowerShell), and
executes only that snapshot. New campaign artifact:
`F:/Temp/rah-codex-certification-snapshot-5964-1791118055856320300-0/codex.exe`,
file ID 26458647811489987, same SHA/326872368 bytes, readonly; full identity stable
after version, adapter and Desktop execution. PIDs 18992 and 2320 exited.
This campaign intentionally retains its persistence root, as before; cleanup
proof comes from the two closure smokes using the preserved snapshot.

Tauri-generated resource.lib linkage remains behind the explicit Windows
certification feature. mt.exe was used only to extract/inspect resource #1, never
to modify the executable. No 0xc0000139; direct executable listing succeeds.

## Feature graphs and metadata

Windows-filtered offline cargo metadata confirms default Desktop features
`default,provider-codex`, adapter `default`: certification disabled. Explicit
Desktop certification graph features `certification-harness,default,provider-codex`
and adapter `certification-harness,default`. Saved in *-windows-metadata.json.
Both graphs contain **14 workspace packages, all 0.33.0, edition 2024**.

Initial unfiltered offline metadata could not download uncached Android-only
android_system_properties 0.1.6. This was a probe limitation, not a source change;
Windows-filtered metadata resolved successfully without downloads.

Source gating retains exact constructor/snapshot/candidate admission in the
non-default adapter certification module; Desktop certification_tests requires
test + Windows + certification feature. Normal graph cannot call these absent
APIs. No frontend/env/config/CLI/Tauri activation control or certification IPC
was added. No default unknown-version admission change.

Cargo.lock SHA remained task-start/Task 510B2L value:
`9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe`.
The existing WIP lock diff adds windows-sys to the adapter; it was preserved.
No dependencies added by M. uuid remains dev-only; snapshot naming remains
std-only PID/timestamp/atomic-counter, no new naming runtime dependency.

## Exact failed gate and stop

No frontend source changed. Started existing syntax/frontend suites in filename
order. status.js node --check passed; model_picker_test.js and
model_preflight_test.js passed. remembered_workspace_layout_test.js failed:

```text
AssertionError [ERR_ASSERTION]
Error: spawnSync C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe ETIMEDOUT
code: ETIMEDOUT; errno: -4039
spawnSync at remembered_workspace_layout_test.js:50:20
assertion at remembered_workspace_layout_test.js:52:12
--headless=new --disable-gpu --no-first-run
--user-data-dir=F:\Temp\rah-remembered-layout-XtxQBr\profile-626
--window-size=626,793 --virtual-time-budget=1000 --dump-dom
file:///F:/Temp/rah-remembered-layout-XtxQBr/layout.html
Node.js v24.20.0; process exit 1
```

Edge also emitted fallback_task_provider.cc:126 renderer task-provider errors.
No retry, frontend mutation, browser timeout change or cleanup suppression.
The remaining frontend suites and tauri_permission_test.js were not reached.
No full frontend/static PASS or Tauri 47/47/47/47/47 claim for this task.

cargo check --workspace had already started before the failure was observed;
it completed PASS, exit 0, in 2m 27s. No validation restart occurred.
No subsequent workspace test/clippy/canonical Desktop/HostExplicit gate was
started after the stop. Workspace passed/failed/ignored totals, canonical Desktop
counts, Tauri executable/static inventory, and HostExplicit executable count
were therefore not established in this task. Historical 47 and 11 are requirements,
not new validation evidence. No staged-file or staged-diff gate was run.

## Authority, ADR and publication disposition

This task changes certification test infrastructure only. No changes to
ToolRegistry authorization, repository authority/switching/leases, permissions,
Trusted Profiles, mutation uncertainty, remembered-workspace behavior or
provider/model authority. Exact-artifact eligibility grants no Tool authority.
No Task 510B runtime/model/Tool certification result. ADR-B: existing ADR 0005,
ADR 0030 and Task 498 error-envelope boundary suffice; no new ADR.

Earlier reports remain unchanged. Complete intended Task 510B2 staging audit and
publication are withheld under F. Committed file count: **0**; no new commit
SHA/message, push or CI run. Worktree remains dirty with preserved WIP and this
report. Final local HEAD and the existing local origin/master ref both equal
`436470337269546a1209d0b9803c2b8aa9e1f6b4`; no remote refresh/publication occurred.
Worktree is not clean. Final status/stat and git diff --check were inspected;
diff check PASS, including the new report checked separately without staging.

Task 510B resume authorization is **WITHHELD**. It requires a subsequent complete
classification A plus natural exact-head CI PASS, and must not begin automatically.
Even later Task 510B success cannot update admission/preferred baseline; Task 510C
owns that separately. Next scope is bounded diagnosis of the failed existing
frontend layout gate, followed by separately authorized closure validation.
