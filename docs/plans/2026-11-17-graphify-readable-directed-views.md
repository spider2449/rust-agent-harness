# Readable directed Graphify views

## Goal and scope

Make the existing Graphify page understandable through explicit arrowheads,
plain relationship labels, focused incoming/outgoing views, and source details.
Use the shared Web/Desktop renderer. The user authorized plan corrections and
execution of P1 through P4 on 2026-11-17. Installation, commits and pushes remain
outside this authorization.

Retain Graphify as an optional experimental graph reference. Presentation does
not repair incomplete extraction or establish reliable navigation. Preserve the
decision in 2026-11-17-graphify-navigation-withdrawal.md and the Python/Qt limits
in KNOWN_ISSUES.md. Missing relationships never establish absence of use.

## Current implementation and architecture

- examples/graphify/graph-view.mjs projects artifact nodes into a bounded graph,
  supports full-artifact search and direct-neighbor focus, and combines relation
  and confidence into an edge label.
- plugins/frontend-web/assets/graph.js owns shared SVG rendering and interaction.
- crates/rah-api/src/ui.rs defines data-only graph vertices and edges. ADR 0079
  limits a graph to 250 vertices and 750 edges; plugins supply no markup or URLs.
- The current graph emphasizes source groups and technical labels. The new view
  must explain relationship meaning and expose direction on the canvas itself.

Keep Graphify-specific relation interpretation in the Graphify process plugin.
Keep reusable geometry, interaction and accessibility in the frontend. Do not
add kernel logic, plugin-to-plugin dependencies, arbitrary file access, or
provider-controlled navigation. Before any rah-api contract change, read ADR
0036 and the pivot plan, then update ADR 0079 or write a successor as appropriate.

## P1: Establish relationship semantics

Inventory emitted relation names and their endpoint conventions using the pinned
extractor implementation and small source-backed fixtures. Record a mapping
table with raw name, source role, target role, readable label and evidence.
Cover calls, crate dependencies, imports, containment, inheritance and interface
implementation only where those relationships are actually emitted.

For a verified call, A -> B reads "A calls B". For a verified dependency it reads
"A depends on B". Do not infer direction from field names alone, reverse a
relation for visual convenience, or reinterpret a generic relation as a call.
Unknown relations retain their raw name and stored source-to-target orientation,
with an explicit "meaning unverified" explanation. Symmetric relations must not
be presented as directional facts without evidence.

Keep raw relation and extraction classification available independently of the
readable wording. EXTRACTED and INFERRED describe provenance, not proof of
correctness. Do not parse semantic state back out of a combined display label.

Acceptance: fixtures establish the mapping and fallback behavior; every supported
label has an unambiguous sentence and endpoint interpretation.

## P2: Draw readable directed edges

Render filled arrowheads for verified directional semantics and hollow arrowheads
for unverified stored orientation, with a textual explanation. Symmetric edges
have no arrowheads and belong in a separately labeled undirected detail section.
Legacy payloads have unverified orientation. Render arrowheads at target boundaries,
outside node fills. Keep arrowheads
visible during zoom, pan, filtering and selection. Use separate curved paths
for reciprocal edges and distinguish parallel relations between the same pair.
Render self-relations with a visible loop and arrowhead. Scope SVG marker IDs
per graph so multiple graph instances do not interfere.

Show readable relation labels on selected or focused edges. At overview scale,
reduce label density without hiding direction. Hover and keyboard-accessible
edge selection expose the full source, relation and target sentence. Provide a
legend explaining arrows, groups and extraction classifications. Use text as
well as visual styling; inferred edges may be dashed but must also be labeled.

Preserve graph bounds, safe text rendering and bounded layout work. Do not add
an unrestricted layout loop or background task.

Acceptance: single, reciprocal, parallel and self edges remain distinguishable;
arrows are not hidden behind nodes or clipped at the viewport. Long labels,
overlapping nodes, dense graphs and multiple graphs have explicit browser cases.

## P3: Focused reading and relationship details

Selecting a node highlights its direct relationships and dims unrelated content.
Provide incoming, outgoing and all-direction filters, plus relation-type filters.
For a calls-only focus, prefer callers on the left, the selected symbol in the
center and callees on the right. A node that is both a caller and callee keeps a
single identity and exposes both directions. Mixed relations retain their actual
semantics; do not label all incoming nodes as callers.

Show two accessible detail lists grouped by relation type. Each item includes
the other endpoint, a complete relationship sentence, raw relation/provenance,
and available source file/line information. Distinguish a node definition
location from evidence for an edge; do not fabricate call-site locations.
Use wording such as "relationships found in this index" and "none found".
State whether these lists describe the rendered subset or the full artifact.
Disclose filtered and capped relationships before P3 closes, including per-list
omissions. "None found" must name the searched subset. Bound list rendering and
provide explicit expansion for relationships outside the current projection.

Keep node selection distinct from explicit neighbor expansion. Add back/reset
navigation and bounded per-session history. Workspace changes reset selection
and history; graph refreshes reconcile deleted nodes and invalidate stale data.
If counts come from the full artifact, identify them as such; distinguish them
from counts after filters and display limits.

Source locations are readable/copyable initially. A clickable source-opening
action requires an existing authorized host capability with workspace/path
validation; otherwise defer it explicitly rather than accepting plugin URLs.

Acceptance: incoming/outgoing lists agree with edge orientation, filtering never
reverses edges, and focus/history remain correct across refreshes and workspaces.

## P4: Progressive overview and completeness

Offer module/package, file and symbol views only when supported by artifact data.
Start with module dependencies where available; otherwise state which fallback
view is shown. Expand deliberately into smaller neighborhoods. Search continues
to cover the full artifact, while rendering remains bounded.

If deriving file/module summaries from symbol edges, label them as aggregated
relations, preserve underlying relation types and directions, and disclose
counts and aggregation rules. Do not turn a call into an import or dependency.
Do not mix incompatible abstraction levels without labeling them.

Display active level, filters, snapshot time, indexing readiness and counts for
the artifact, current query and rendered subset. Disclose every cap or omitted
subset, including unavailable totals. Maintain the existing byte budget as well
as node/edge bounds. Keep known extraction limitations visible in the page.

Acceptance: users can explain which subset they are viewing and why a matching
node or edge might not be drawn. Stale, empty, unsupported and failed views have
specific messages and do not imply complete analysis.

## Contract and compatibility decision

During P1, determine the smallest reusable optional metadata needed for relation
kind, provenance, direction semantics, source details and view mode. Reuse
existing nodes/forms/tables where practical. Do not hard-code Graphify relation
names into the renderer or overload display strings as a protocol.

If additive graph fields are necessary, define bounded typed data, defaults,
validation and old-payload behavior in rah-api. Update ADR and compatibility
tests before consumers emit new fields. Keep process protocol methods unchanged
where possible, and preserve the existing status-only/older-host fallback. New
metadata must not grant permissions or introduce executable content.
Existing graph hosts reject unknown fields. Gate new metadata with an explicit
`graph_metadata` configuration flag, default false; only a compatible host install
may enable it. `graph_view = true` alone continues to emit the original graph
schema. Test that metadata-disabled output deserializes with that schema and that
metadata-enabled output passes the new host's validation. Older hosts without
graph support keep `graph_view` false. Do not assume optional fields are accepted
by an older reader.

## Validation and delivery

Implement and validate P1 through P4 sequentially; each phase must meet its own
acceptance criteria before moving on. The user's execution authorization covers
all four phases; record any acceptance criteria that cannot be verified.

- Deterministic Node fixtures: relation mapping, unknown/symmetric relations,
  projections, direction/relation filters, aggregation, caps, stale data,
  selection history and workspace changes. No live model or network required.
- API/process tests when contracts change: optional defaults, old payloads,
  invalid metadata, resource limits and safe fallback behavior.
- Real browser tests: arrow geometry and screenshots for all edge shapes,
  zoom/pan, reciprocal labels, keyboard detail access, focus/filters, multiple
  graph instances, text injection, and maximum supported graph size. Inspect
  screenshots; DOM assertions alone do not establish visual readability.
- Verify shared Web and Desktop appearance. Record native Desktop verification
  separately; a browser pass does not establish native inspection.
- Run relevant Node and Rust tests per phase. At the completed feature boundary,
  run cargo fmt --check, cargo check --workspace,
  cargo test --workspace --no-fail-fast,
  cargo clippy --workspace --all-targets --all-features -- -D warnings,
  and git diff --check. Serialize Cargo commands to avoid workspace lock contention.

Update user documentation and record unresolved issues in KNOWN_ISSUES.md.
Before closure inspect git status --short, git diff --stat and git diff --check.
Report changed files, executed validation, ADR/contract impact, Git state,
remaining limitations and the next phase. When deployment is separately
authorized, refresh compatible embedded assets and installed scripts and state
whether a rebuild/restart is needed. Do not claim activation from source edits.

## Suggested first implementation task

Implement P1 and P2 as the first coherent change: verified relationship wording
and visible, correct arrow geometry with deterministic and visual coverage.
Leave focus/history and progressive overview to their own subsequent phases.

## Execution record (2026-11-17)

The user subsequently authorized execution of all four phases in this task.
P1 added a pinned-extractor endpoint table, offline Python source fixtures and
Node wording/fallback checks. P2 added bounded curved paths, target markers,
self/coincident loops, marker isolation and keyboard edge details. The P2 browser
test passed and its shape/maximum-size screenshots were inspected before P3.
P3 added rendered-subset direction/relation filters, grouped incoming/outgoing
and symmetric details, definitions distinct from edge evidence, caller/callee
placement, explicit expansion and bounded selection/query history. Its browser
and Node checks passed before P4. P4 added evidence-gated package/file/callable
symbol levels, mixed-artifact fallback, aggregation rules and counts, full-artifact
search, and cap/limitation disclosures. Module-only view is explicitly unsupported.

ADR 0079 now specifies optional bounded edge explanations and an opaque graph
revision. `graph_metadata` defaults false, independent of `graph_view`; the
compatible installer enables it. No kernel, permission, process-method or crate
dependency changes. Source opening remains deferred; locations are text only.
The graph byte cap is 140 KiB, leaving room for forms and status in the existing
256 KiB view budget. Histories and provider sessions each retain at most 32 entries.

Native Desktop visual acceptance remains pending and is recorded in
KNOWN_ISSUES.md. Browser screenshots do not claim native inspection. Sources and
embedded asset definitions changed; installation, activation, commit and push
were not performed. Final validation results are recorded below after execution.

Executed validation:

- `node --test examples/graphify/*.test.mjs`: 36 passed.
- Pinned local Python: `python -B examples/graphify/relations.test.py`: passed.
- `cargo test -p rah-api ui::tests:: -- --nocapture`: 11 passed.
- Real headless Edge: `cargo test -p rah-cli --test web_browser
  relationship_graph_supports_inspection_filter_zoom_pan_and_expansion --
  --exact --nocapture`: passed. Screenshots of edge shapes, maximum-size graphs
  and the existing workspace's aggregated file view were inspected. Repeated
  identical graphs also survive the shared frontend's reuse path independently.
- `cargo fmt --check`, `cargo check --workspace`,
  `cargo test --workspace --no-fail-fast`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and
  `git diff --check`: passed. Workspace tests keep their existing opt-in live
  checks skipped; this task used no live model or fresh network extraction.

Next acceptance task: native Desktop inspection after a separately authorized
build/install refresh. Dense overviews remain bounded references rather than a
complete readable map; select a node, narrow filters or expand a neighborhood.

Follow-up: node dragging is implemented in the shared renderer. Pointer capture
and inverse screen coordinates preserve dragging after zoom/pan; paths and arrows
update immediately. Manual positions survive selection/filtering until graph
replacement or Reset positions. A drag does not trigger selection or expansion.
The real Edge test verifies movement after zoom/pan, updated paths, unchanged
viewport/selection, retained positions and reset. It passed, along with the Node
geometry test, JS syntax check, cargo fmt --check and git diff --check. This
follow-up changes no API/ADR contract and has not been installed or committed.

Commit authorization: the user reported testing OK and authorized committing
the complete directed-view and dragging change. This does not identify the
tested runtime, so the separate native Desktop inspection note remains open.
No push or installation was requested.
