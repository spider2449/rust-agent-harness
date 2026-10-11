# Task 510B2N — Remembered-workspace Edge timeout and seam publication closure

Reference-only follow-up: [Task 510B2O](2026-10-04-task-510b2o-certification-seam-clippy-recovery-and-publication-closure.md)
corrected the nine adapter findings; focused Clippy passed, then workspace Clippy
reached pre-existing Desktop drop_non_drop at certification_tests.rs:226 and
stopped under D. Historical N evidence and classification below remain unchanged.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.

Plan: preserve M evidence; inspect exact test and frontend diff; execute the unchanged
isolated test once and, on PASS, once more; classify before resuming workspace,
clippy, canonical Desktop, frontend/static, Tauri, metadata, HostExplicit,
snapshot and authority gates. Publish only under final A with natural exact-head CI.
No Task 510B execution, admission or preferred-baseline change.

## Timeout diagnosis

Original command: `node crates/rah-desktop/frontend/remembered_workspace_layout_test.js`
from `F:/coding/otherPrj/rust-agent-harness`; Node 24.20.0, exit 1.
M's original report and evidence directory remain preserved. No separate original
frontend failure log exists in that directory; its exact failure is recorded in M.
Edge spawnSync ETIMEDOUT (errno -4039), first 626×793 fixture,
`F:/Temp/rah-remembered-layout-XtxQBr/profile-626`, 30000ms timeout.
Renderer fallback_task_provider.cc:126 messages were recorded by M.

Line 50 invokes Edge; line 52 asserts result.error is undefined. The limit applies
to synchronous process completion across launch/load/script/dump/exit, not a
selector or layout assertion. Original evidence cannot distinguish the internal
browser phase. The fixture sets data-layout-result after a zero-delay timer,
using virtual-time-budget=1000; subsequent assertions inspect actual rectangles,
overflow, enabled buttons, picker result, and scrolled action reachability.
It launches Edge itself, with mkdtemp and separate fresh profile per viewport;
no persistent/reused browser profile. Thus reused-versus-fresh control is inapplicable.
It exercises production shell/CSS/editor construction with test-local callbacks.

Frontend diff against HEAD: empty. No relevant product, CSS, viewport, startup,
or asset-loading changes in the WIP. No frontend/test correction made.
Edge executable: `C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe`,
file/product version `154.0.4258.53`.
No Edge processes before the first run or after either run. During first run,
five identifiable fixture-owned processes were captured at the third viewport;
no process cleanup/kill was needed. Evidence: `F:/temp/task510b2n-evidence/`.
Initial free memory 49,987,320 KiB of 67,020,808 KiB, 204 processes;
F: free 810,582,822,912 bytes; sampled CPU 4%. No resource-pressure evidence.

Isolated unchanged commands, same cwd/timeout/source/environment:

- isolated1.log: PASS, exit 0, 37276ms for three fixtures.
- isolated2.log: PASS, exit 0, 32116ms for three fixtures.

Both include 626×793, 506×653 and 476×633 with all existing DOM/layout assertions.
No isolated failure; no missing expected DOM state. Console/page-error instrumentation
is absent in the existing harness; passing assertions do not certify absence of
all console errors. No DOM/screenshot capture after a timeout was available.

Intermediate classification: **H1 — TRANSIENT EDGE EXECUTION FAILURE**.
Two consecutive isolated PASS, no stale-profile/process dependency found, no
product defect observed. H2/H3/H4/H5 are unsupported by the measured evidence.
No timeout inflation, browser version change, or source correction.

## Closure results

Final Task 510B2 classification: **D — FULL WORKSPACE/CLIPPY/DESKTOP GATE FAILED**.
M focused seam gates remain valid; not rerun unnecessarily.

Both target variables were `F:/temp/rah-task510b2g-target`. Source was frozen
throughout both resumed commands. Evidence directory contains complete logs.

| Gate | Result |
| --- | --- |
| `cargo test --workspace` | PASS, exit 0; 1085 passed / 0 failed / 24 ignored; 58 summaries |
| Desktop binary within workspace | 358 passed / 0 failed / 20 ignored; 257.63s; not canonical harness closure |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | FAIL, exit 101; nine errors in adapter WIP |
| Canonical Windows Desktop harness | Not run after clippy failure |
| Full frontend/static closure | Not run after clippy failure; only two isolated layout executions, three fixtures each, PASS |
| Tauri 47/47/47/47/47 | Not rerun; no current closure claim; no certification IPC in inspected diff |
| Metadata 14 packages, 0.33.0, edition 2024 | M evidence preserved; not re-executed after stop |
| Cargo.lock | SHA256 `9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe`, unchanged from task start |
| HostExplicit | Existing static assertion exactly 11 inspected; `rename_file_is_the_eleventh_host_tool_and_requires_rename_authority` PASS within workspace; separate closure gate not run |
| git diff --check | PASS |

Clippy's exact failing sites (no correction or retry):

- `certification_support.rs:102,225,312,392`: `result_large_err`.
- `certification_support.rs:400`: `io_other_error`.
- `process.rs:69`: `needless_borrows_for_generic_args`.
- `process.rs:95,99,103`: `needless_borrow`.

The large error variants preserve measurements inline (192–384 bytes); the
process warnings arise from borrowing the already borrowed executable parameter
in start_verified. This is a certification seam lint blocker, independent of
the isolated frontend timeout. Full diagnostics are in `clippy.log`.
No lint suppression, semantic change, source correction, or further gate retry.

## Snapshot, authority and ADR

Independent final Get-FileHash/Get-Item/fsutil measurement:
SHA256 `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`,
length 326872368, file ID `0x0000000000000000002e0000000a80a1`;
readonly/archive. Existing preserved snapshot intact; not recreated or committed.
No staging took place, so this is final diagnostic measurement, not publication.

This task changed only this report and a reference-only M note. No change to
certification semantics, ToolRegistry, repository authority/switching, permissions,
leases, Trusted Profiles, mutation uncertainty, remembered-workspace authority,
runtime admission or preferred baseline. Inspected complete tracked WIP diff:
certification feature/verification/test integration only; existing snapshot/typed
support and M evidence remain preserved. No newly added dependencies or lock drift.
Existing WIP windows-sys dependency remains as documented by M.
0.160.0 remains unadmitted/uncertified; preferred 0.157.1 unchanged.
ADR-B: ADR 0005, ADR 0030 and Task 498 diagnostic envelope remain applicable;
no new ADR. Full publication authority/security closure remains withheld.

## Publication disposition

Stopped at clippy failure under D. Committed file count **0**; no new commit
SHA/message, staging, push or natural exact-head CI. Starting and final HEAD
and local origin/master equal `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
No remote refresh was performed. Worktree remains dirty with complete unpublished
WIP and reports; no clean-worktree claim. Original M failure output unchanged;
its sole report update is a reference-only note pointing here.

Task 510B resume authorization **WITHHELD**. Do not start Task 510B automatically.
Next bounded task: resolve the nine seam clippy findings without changing
certification semantics, then resume outstanding closure gates. Only final A
and successful natural exact-head CI can authorize Task 510B resumption.
