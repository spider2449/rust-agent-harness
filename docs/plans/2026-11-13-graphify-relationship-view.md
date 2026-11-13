# Interactive workspace relationship view

## Scope

Draw the active workspace's Graphify graph inside the existing Graphify plugin
page. Support pan/zoom, group filtering, search across the full artifact, node
details and expansion of direct relationships. Keep indexing status/footer and
prior workspace-consumption changes intact. Clearly label bounded subsets,
inferred relationships and stale/unavailable snapshots.

## Design

Add a reusable data-only graph node to UiViews (ADR 0079), with validated bounded
vertices/edges and an optional user-triggered expansion action. frontend-web
owns SVG rendering and interaction; plugins supply no HTML, scripts or URLs.
Graphify's process view reads its current workspace graph through existing
status ownership and artifact validation, caches by file revision and exposes
full-artifact search plus bounded projections. A configuration opt-in preserves
compatibility with old hosts. No Graphify logic enters the kernel/frontend.

## Validation

Test API bounds and compatibility, graph projection/search/focus/workspace
switches, process protocol and a real headless browser for graph interactions
and safe text rendering. Extend the native cached Graphify integration with
render/action checks. Run formatting, targeted Rust/Node checks and diff checks.
Deploy compatible scripts/binaries when safe without interrupting user sessions;
record any activation limit. Do not commit.

## Result

Implemented a generic UiViews graph node and the shared frontend SVG renderer.
It supports zoom, wheel navigation, drag panning, reset, an expanded view with
Escape dismissal, source-group checkboxes, a keyboard-accessible node list,
local search and node/edge details. Host-owned layout has fixed work bounds and
all provider content is rendered as text. Unchanged graphs retain viewport and
selection when the surrounding status page refreshes.

Graphify's graph-view.mjs reads/validates the current workspace artifact, caches
its revision and produces bounded projections. The initial RAH view shows crate
dependencies; full-artifact search and explicit node expansion expose symbol
neighborhoods. Views disclose shown/total counts, generation time, confidence
labels and last-snapshot warnings. Session searches are separate and reset on
workspace changes. Deleted focused nodes fall back to the overview. The graph
appears above existing indexing details on the same page/footer entry point.

The real workspace preview contained 39 crate vertices and 104 dependency edges,
drawn from a graph of 6587 vertices and 18568 edges. This is deliberately a
declared projection, not an assertion that only those crates or relationships
exist. Source inspection remains necessary for inferred relationships.

Validation actually executed:

- 29 deterministic Node tests passed, including full-artifact search, group
  selection, focus, per-session/workspace state, deletion, corrupt artifacts,
  long multibyte data within byte bounds, and the opted-in process protocol.
- 10 rah-api UI tests passed, including graph structure/text/resource bounds.
- Real cached Graphify through RAH passed (17.63 s, mock model): authenticated
  render, graph search/focus actions, MCP evidence, source read, change/recovery,
  workspace switching and shutdown.
- Real headless Edge interaction test passed (2.78 s): actual viewNode renderer,
  text safety, node inspection, zoom, drag pan, filters, expansion and callbacks.
  The actual workspace projection screenshot was inspected at
  target/graphify-ui/graphify-workspace-relations.png. This is an isolated browser
  test, not inspection of the user's existing Desktop window.
- Web API tests: 34 passed, one synthetic performance test ignored. Process
  plugin and architecture suites: four passed each.
- cargo check --workspace, cargo fmt --check, targeted all-target Clippy with
  -D warnings, JS syntax and git diff --check passed. CLI/Desktop release build
  succeeded. Full workspace tests were not run.

## Installation and activation

Updated the installed CLI/Desktop binaries under
C:\Users\spider.tp\AppData\Local\Programs\RAH and all five Graphify modules under
C:\Users\spider.tp\.rah\plugins\graphify. Binary and script hashes match their
build/source. Added only graph_view = true to the existing status-view mount;
reversing that one text change reproduces the original rah.toml exactly. Staged
and installed configuration checks accepted 23 plugins with a one-run mock
override; the saved model and other settings were preserved.

Backup: C:\Users\spider.tp\.rah\config-backups\20261113-092152-graphify-relationship-view.
The loaded old executable was moved into the backup before publishing the new
binary. Desktop PID 17736 continued running; no session or application was
stopped. The running old host keeps its existing UI until normal restart.
After restart, open Plugins > Plugin pages > Graphify or click its footer icon,
then Expand view for a larger canvas. No commit was created and prior workspace
consumption changes remain intact.

ADR 0079 records the additive UI vocabulary. Kernel, service trait signatures,
process protocol methods, tool permissions and crate dependency edges did not
change. Suggested next user check: restart normally, search a familiar symbol,
select its node and Explore connections.
