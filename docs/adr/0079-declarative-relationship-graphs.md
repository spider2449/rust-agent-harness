# ADR 0079: Declarative relationship graphs in plugin views

Status: Accepted

Date: 2026-11-13

## Context

Graphify's workspace relationships need an interactive view. UiViews currently
supports tables but no spatial relationships. Embedding plugin HTML or adding
Graphify-specific routes would violate the fixed, data-only view boundary.

## Decision

Extend the UiViews node vocabulary with graph: a title, bounded vertices
(unique id, label, group, detail), edges (source, target, label) and an optional
select_action. Limit each graph to 250 vertices and 750 edges, with short text
fields; existing view node/byte limits still apply. Reject duplicate ids and
dangling endpoints. Fields are text only, never markup, styling or URLs.

The frontend owns SVG layout, zoom/pan, group colors/filtering, visible-node
search and a keyboard-accessible node list/detail display. It only calls the
provider's optional action when the user explicitly requests expansion, passing
{"node": "id"}. It does not navigate to provider-supplied paths or execute graph
content. Providers own full-corpus queries, graph sampling and source semantics.

This additive data extension keeps UiViews traits and process protocol 4 methods
unchanged. Existing views remain valid; providers emit graph nodes only on hosts
supporting the extension. Graphify enables them through graph_view configuration
and keeps its legacy status-only mode for older hosts. No kernel, model tool,
permission or crate dependency edge changes are required.

## Consequences

Graphs are reusable by other plugins and have fixed rendering/resource bounds.
Large corpora must expose bounded projections and disclose omitted data; they
cannot silently imply the whole workspace is visible. Browser and API tests
cover interaction, text safety, bounds and view lifecycle.

## Directed relationship explanations (2026-11-17)

Edges may optionally contain `metadata`: relation and provenance (128 characters
each), direction (`directed`, `symmetric`, `unverified`), sentence and detail
(512 characters each). All are text-only explanations; source references remain
copyable text with no navigation capability. No permission or protocol method
changes. Graph and enclosing view resource limits still apply.

Absent metadata means unverified stored endpoint order. Filled arrowheads mean
provider-verified endpoint semantics, hollow arrows mean unverified stored order,
and symmetric edges have no arrows. This does not verify an extracted fact.
Providers interpret relation names; renderers never parse labels for semantics.

Older graph hosts reject unknown fields. Graphify's `graph_metadata` flag defaults
false independently of `graph_view`; only installations built with this extension
enable it. Leaving it false emits the old edge schema. Hosts predating graph
support must also leave `graph_view` false. This is explicit configuration gating,
not automatic capability negotiation.

The graph may also carry an optional opaque `revision` (128 characters). Providers
change it across workspace, session or snapshot changes to invalidate local
selection/filter/history state, even when the projected vertices are identical.
It follows the same compatibility gate as edge metadata.
