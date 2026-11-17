# Withdraw Graphify navigation positioning

## Scope and decision

Withdraw the reliable code navigation positioning after the user reported
missing member access, misclassified properties, missing Qt signal/slot wiring
and unresolved instance-attribute calls. Retain the optional integration as an
experimental graph reference. Remove graph-first tool guidance, make source
search and inspection primary, and disclose the limits in discovery, query
results and user documentation. Preserve unrelated working-tree UI changes.

## Validation and impact

Update deterministic MCP discovery/result assertions and run all Graphify Node
suites plus git diff --check. No kernel, ADR, service contract, protocol or
permission changes. No extractor repair or fresh quality evaluation is claimed.
Historical evaluation results remain historical; the earlier tool-selection
plan is explicitly superseded.

Existing installations require refreshed artifact.mjs and an MCP restart;
embedded assets require a new RAH build and installation refresh. This task
changes repository sources only. The user subsequently authorized the scoped commit; no deployment or push is included.

Validation result: all 32 tests passed across artifact, managed MCP, status and
graph-view suites. git diff --check passed. Rust workspace checks were not run
because no Rust source or contracts changed. Existing UI changes were preserved.
