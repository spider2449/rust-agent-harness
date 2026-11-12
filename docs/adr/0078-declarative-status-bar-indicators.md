# ADR 0078: Declarative plugin status-bar indicators

Status: Accepted

Date: 2026-11-12

## Context

Graphify contributes a status page, but background indexing should be visible
without opening that page. Hardcoding its process id, file format or states in
frontend-web would couple the frontend to one integration and contradict ADR
0036. ADR 0062 already provides bounded, data-only views and change notifications.

## Decision

Extend ViewSpec with `status_bar`, default false. An opted-in view may return
one `status` node with a closed built-in icon (`activity` or `graph`), a short
label, a semantic state (`idle`, `busy`, `ready`, `warning`, `error`) and tooltip
detail. View bounds also limit label/detail length and reject duplicate nodes.

Frontend-web renders at most four indicators in its own footer container. It
does not accept markup, style, URLs or arbitrary click actions. Clicking opens
the contributing view. Existing workspace, Git, model, tool/guard and usage items
remain frontend-owned; layout plugins still cannot move or hide them.

The frontend fetches opted-in view state on discovery and UiChanged events even
when the detail page is closed. Removed providers disappear. Failed renders or
connection loss show a warning instead of retaining an apparently ready state.

This is an additive rah-api data-vocabulary change. No kernel/service trait or
tool permission changes are needed. Existing ViewSpec JSON and views remain
valid. Process protocol 4 keeps the same methods; providers must opt in to the
new metadata/node only on an updated host. Graphify uses a configuration flag
so its legacy detail page remains compatible with older hosts.

## Consequences

Tests cover API bounds and legacy defaults, authenticated view discovery,
browser updates/click/removal without opening the page, and the Graphify
provider's health/progress mapping. Frontends that do not render indicators
may ignore the status node. No plugin is given browser code or control over
security UI.
