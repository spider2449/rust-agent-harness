# Task 515 — Desktop workspace redesign

Starting checkpoint: 0818efbf999f3592bf7195fcd05578e2bb69526a.
Local HEAD, origin/master and live GitHub master agree; CI 37872597531 succeeded.
Release preparation is deferred. Historical untracked 514/514C reports are preserved.

## Implementation and validation plan

1. Inventory original HTML IDs, forms, dialogs, JS handlers and nine suites.
2. Reorganize existing nodes into toolbar, Workspace, Chat and tabbed Inspector.
3. Add bounded pointer/keyboard splitters, collapse/restore, responsive overlays,
   reset and a separately versioned presentation-only localStorage preference.
4. Test production layout functions and stylesheet in Edge and retain existing suites.
5. Run requested Rust, inventory and production build gates, then actual Windows
   WebView2 acceptance with screenshots and owned process evidence.
6. Commit/push only after successful validation; require exact-head CI.

## Storage review

Tauri uses frontendDist=frontend and its normal WebView2 origin/profile; no custom
WebView data directory is configured. Backend model/identity preferences use
Desktop app-local storage. Layout uses a separate localStorage key and closed
schema containing only sizes, collapsed flags and a fixed tab identifier.
No backend preference schemas, permissions or authority interfaces change.

## Implementation checkpoint

The production entrypoint now loads layout.js before status.js. It moves existing
DOM nodes into a compact toolbar, Workspace navigation, flex-column Chat and a
five-tab Inspector. Original handler IDs remain; repository stage/unstage delegation
stays on its original repository ancestor. Connection errors sit beneath the
toolbar; selection errors sit in Workspace. Native chat prompts no longer say Codex.

Splitters use pointer capture, bounded outer panel widths, pointerup/cancel/lost
capture cleanup and Arrow/Home/End keyboard controls with ARIA separators.
Workspace minimum is 180px, Inspector 260px, Chat 320px on desktop. Collapse removes
the panel track and retains its custom width. Focus Chat collapses both sides;
Reset Layout restores 240/340px. Below 960px, Chat takes the main area and toolbar
toggles expose side drawers. Resizing into narrow mode collapses the drawers.
Layout schema v1 persists only bounded widths, collapsed booleans and one fixed
tab identifier; malformed/unknown/out-of-range data uses defaults. Storage errors
leave the interface usable. Browser reload persistence passed; actual Desktop
restart persistence remains unverified.

Changed frontend files: index.html, status.js, styles.css; new layout.js and
workspace_layout_test.js. No backend, dependency, ADR, Tauri permission, version,
model/identity schema, ToolRegistry, HostToolPort or host authority changes.

## Executed validation and blocking failure

Evidence directory: target/task515 (ignored, retained).

| Gate | Actual result |
| --- | --- |
| All frontend JS syntax | PASS |
| Existing nine frontend suites | PASS; eight non-browser suites rerun after chat text change |
| New production-function/stylesheet Edge suite | PASS at 1600/1200/626/476px |
| Pointer handlers | PASS: lower/upper bounds, capture recording, cancel/up cleanup, inactive moves; synthetic capture stub documented in test |
| Keyboard/collapse/reset/tabs | PASS |
| Layout reload / corrupt preference fallback | PASS in Edge |
| Narrow Chat/composer, overflow, built-in dialogs | PASS in Edge |
| Tauri command inventory | PASS: 49 runtime/manifest/generated/default/frontend |
| Native adapter tests | 23 PASS |
| Neutral runtime tests | 18 PASS |
| Production OpenAI host Tool fixture | 1 PASS |
| Fixture helper build | PASS |
| Workspace tests | FAIL, exit 101; stopped during Desktop suite |
| Desktop within workspace | 338 passed, 1 failed, 13 ignored |
| git diff --check | PASS |
| Canonical Desktop standalone / fmt / check / Clippy / production release build | NOT RUN after required workspace failure |
| Actual Windows production WebView2 acceptance / screenshots | NOT RUN |

Failure: tests::repository_snapshot_matrix_isolated_repositories_and_replacements
panicked at crates/rah-desktop/src/main_tests.rs:2295, unwrapping
Err(StagedDiffExecution) while observing selected repository C's staged diff.
The existing desktop_repository_snapshot_internal path maps staged-diff Tool
execution failures to this stage. The underlying Tool error is not retained in
this panic; timeout, load, filesystem, or other causes are not established.
No test weakening, deadline change, backend repair, retry-to-green or baseline
causal claim was made. The gate runner stopped on exit 101. No Cargo or Desktop
process remained at closure inspection.

## Feature parity and actual acceptance limits

| Feature | Implementation / evidence |
| --- | --- |
| Native OpenAI/llama.cpp and optional Codex configuration | Existing controls retained in Runtime / Model; configuration suites PASS |
| Repository admission/switch/close | Existing controls retained in Workspace; membership suite PASS |
| Remembered workspace editing | Existing forms/dialogs retained in Workspace; functional/browser suites PASS |
| History/context, Send/Cancel, error handling | Existing handlers retained; lifecycle suite PASS; actual model turns NOT VERIFIED |
| Effective vs advertised Tools and Host action distinction | Existing Authority section unchanged; authority suite PASS |
| Status/Diff/Stage/Unstage/staged and commit reviews | Existing repository ancestor/controls retained in Status / Review; actual UI workflow NOT VERIFIED |
| Trusted Profile selection/restore and commit identity | Existing controls retained in Settings; actual redesigned UI workflow NOT VERIFIED |
| Model-selected Tool execution, cancellation/recovery, reconnect, missing-key recovery | Production fixture only; actual redesigned Windows acceptance NOT VERIFIED |

HostExplicit remains exactly 11 in unchanged host_invocation.rs; its exact allowlist
test passed within the workspace Desktop output. No frontend state grants authority.
No redesigned Windows screenshot or live chat/Tool acceptance PASS is claimed.
Prepared private CDP scripts and disposable fixture are retained but were not run.

## Disposition

**F — VALIDATION INFRASTRUCTURE BLOCKER**, conservatively a required validation-gate
blocker with underlying cause unproven; no demonstrated UI or authority regression.
This is an uncommitted implementation checkpoint, not Task 515 completion or A.
Starting and final HEAD: 0818efbf999f3592bf7195fcd05578e2bb69526a.
No commit/push/new CI, tag, release, version bump or release artifact preparation.
The starting exact-head CI PASS does not validate the dirty frontend patch.

Historical untracked reports remain untouched with hashes:
Task 514 DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43;
Task 514C A6C834178A8428F3EBD3F3F99A4B254B639DFA1A9A6BEEA3E3B326AD8CAE277F.

Next work: establish the staged-diff failure cause with controlled evidence, then
finish canonical/generic Rust gates and normal production build; run owned Windows
acceptance including actual restart, model Tool/Cancel and recovery before publication.
Remaining UI review includes real WebView screenshots, desktop focus behavior and
provider/model toolbar accuracy under live status transitions. Release preparation
remains deferred and may not resume on this incomplete Task 515 checkpoint.
