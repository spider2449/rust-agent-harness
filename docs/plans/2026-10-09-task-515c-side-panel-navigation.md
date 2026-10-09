# Task 515C � Side panel navigation addendum

## Scope and plan

Extend the existing uncommitted Task 515/515B frontend checkpoint in place.
Preserve all original controls, IDs, delegated ancestors and IPC handlers.
1. Add sticky section navigation using current DOM headings.
2. Add independent disclosure buttons and panel/tab-scoped Expand/Collapse All.
3. Restore panel, select tab, expand target and scroll only the owning panel.
4. Keep expansion ephemeral; retain rah.desktop.layout.v1 unchanged.
5. Extend the existing frontend browser suite without weakening assertions.
6. Build and verify the actual Windows production Desktop using owned WebView2
   input, screenshots and retained-process exit evidence.

## Implementation

layout.js reuses Workspace membership, Remembered Workspaces and conversation
nodes. Inspector tabs and their existing labels/IDs remain unchanged. Changes
uses Status, Worktree Diff, Staged Diff, Staged Review; Tools uses Effective Tools,
Unavailable capabilities and Advanced context. Existing Host actions remain in
Effective Tools; no duplicate action or handler is introduced.

Heading buttons expose aria-expanded/aria-controls. Quick navigation supports
native Enter/Space plus Arrow/Home/End focus movement. Target headings receive
focus without browser scrolling; panel.scrollTo performs immediate panel-only
scrolling, including reduced-motion configurations. Active links use aria-current
and sections have visible markers. Compact wrapped buttons retain narrow access.
No URL/hash, backend preferences, repository state or authority changes.

## Validation checkpoint

Expanded workspace_layout_test.js: PASS at 1600/1200/626/476 pixels.
Initial implementation failures were diagnosed and corrected: detached-panel
queries and preservation of the legacy model section ID. Existing test assertions
remain intact. Production build and Windows verification results follow below.
Evidence directory: target/task515c; prior Task 515/515B evidence preserved.

No dependencies, ADRs, IPC, permission, provider, ToolRegistry or native runtime
changes. HostExplicit remains 11. No staged-diff diagnostic work, commit, push,
release, tag or version bump is authorized or performed.

## Final production acceptance

- Existing ten frontend suites (including extended layout suite): PASS; unchanged
  Tauri inventory PASS (49 runtime/manifest/generated/default/frontend commands).
- Final section/layout browser suite after visibility/details fixes: PASS at 1600/1200/626/476px.
- `cargo build -p rah-desktop --release`: PASS, retained Process exit 0;
  three existing backend compiler warnings reported, no backend changes.
- Actual Windows production WebView2: PASS, final session navigation-final.
  Real CDP mouse/keyboard input verified section anchors, keyboard End navigation,
  independent collapse, tab-scoped Expand/Collapse All, Chat focus exit,
  panel collapse/restore and narrow drawers at 626/476px. Original DOM IDs remain
  present. Screenshots retained for desktop Changes/Tools and narrow panels.
- Final executable SHA256:
  5437092ED51361212CFA7C971166457A1019369117D23769317F538C95768A46;
  length 21178368; PID 2472; start 2026-10-09T11:24:55.9301391+08:00.
  Launch listener ancestry/window identity is in navigation-final/ownership.json
  and launch.json. Normal native window close; same retained Process exit 0
  recorded in navigation-final/exit.json.
- `node --check layout.js` and `git diff --check`: PASS.

This is navigation-only production acceptance, disconnected and without live
inference or host action execution. It does not close the broader Task 515B
observer/lifecycle blockers or claim full Task 515 acceptance. The existing
visible null notice is preserved and not diagnosed here. Generic Rust tests,
fmt/check/Clippy and staged-diff diagnosis were not restarted for this frontend
addendum. HostExplicit source remains unchanged at 11.

Changed by this addendum: layout.js, styles.css, workspace_layout_test.js and
this acceptance file; prior dirty index.html/status.js and historical acceptance
records are preserved. HEAD remains 0818efbf999f3592bf7195fcd05578e2bb69526a;
implementation is uncommitted. No publication, new CI, release/tag/version change.
Next task: separately complete the outstanding Task 515C/515B acceptance gates
under their own authorization; this addendum does not expand that diagnosis.
