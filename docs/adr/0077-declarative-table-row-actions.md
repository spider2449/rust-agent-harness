# ADR 0077: Declarative table row actions

Status: Accepted

Date: 2026-11-11

## Context

The personal plugin library renders a table followed by a second list of
management buttons. The user must match names twice. The UI vocabulary of
ADR 0062 has no way to select a table row, so this cannot be fixed by the provider
alone. The requested compact management screen needs an action on the row.

## Decision

`rah_api::ui::Node::Table` accepts an optional `row_actions` array, aligned with
its rows. An entry is null for a plain row, or the provider action to invoke when
the first cell's name is selected. The frontend renders that cell as a button
and uses the existing authenticated view-action endpoint, with null input.
This grants no additional authority and is never a model tool.

The array defaults to empty and is omitted when empty. Existing table payloads
keep working on the updated host. New row-action payloads require an updated
host and frontend. Nonempty arrays must match the row count; actionable rows
need a nonempty first cell and action. Actions count toward existing view bounds.

My plugins uses this reusable contract, with plugin names and statuses in its
library. Import and backup/export/restore move to separate provider pages.
Selected-plugin details retain the confirmations and operations of ADR 0073.

## Consequences

There is one list and no separate management button per plugin. Other providers
can use selectable rows without frontend special cases. This adds one optional
field to the UI service's serialized data; the kernel, service-call envelopes,
process protocol version and lifecycle/policy contracts stay unchanged.
