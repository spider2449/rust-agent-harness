# Task 299 — Reviewed Rename HostExplicit Coordinator Integration

## Scope

Integrate the Task 298 `RepositoryRenameFilePreparer` with the existing
Windows Desktop HostExplicit coordinator. The route is host-initiated and
ticket-based; it does not add frontend controls or a second rename authority.

## Coordinator flow

The selected repository composition retains the rename preparer alongside the
existing reviewed mutation preparers. A closed `{source_path,
destination_path}` Prepare request passes only those paths to the preparer.
Prepare remains observation-only and stores the private preparation, bound
preparer, exact host-derived ToolCall, current ToolDefinition, registry,
permissions, repository identity, composition identity, and generation tuple in
the existing process-local opaque ticket.

Confirm consumes the ticket and checks currentness, the exact host-owned
repository Tool source, definition, explicit permission membership, retained
preparer identity, capability-specific revalidation, and D2 before crossing
the effect boundary. It then invalidates reviewed Commit authorization for the
selected repository, emits the ordinary HostExplicit `Started` event, and
dispatches the retained ToolCall exactly once through the current
`authorized_tool_dispatch` path.

## Result and privacy boundary

After dispatch, the retained rename preparer performs strict ordinary
`ToolOutput` classification plus fresh independent proof. Only
`ReviewedSuccess`, `KnownNoEffect`, or `Uncertain` are accepted as reviewed
outcomes; malformed, lost, contradictory, or unproven results remain
uncertain. Terminal activity is status-only and never persists the ticket,
private preparation, ToolInput, source content, hashes, native paths, or
filesystem/Git identities.

Prepare, Cancel, and all pre-Started rejection paths preserve reviewed Commit
authorization. Reaching `Started` invalidates it before dispatch, including
dispatch failure and uncertain outcomes. No path retries, replays, reverses,
rolls back, compensates, calls native filesystem APIs, or invokes `git mv`.

## Deterministic evidence

Focused Desktop tests cover the single added eligibility, source and
preparer requirements, closed request/privacy behavior, ticket/currentness and
mutual exclusion, D2 and permission ordering, Commit invalidation timing,
single dispatch, retained ToolInput/registry use, cancellation, strict result
classification, independent success/no-effect proof, uncertainty, and
repository lifecycle invalidation. Existing `rah-tools` reviewed rename tests
remain the foundation for preparation/revalidation/proof behavior.

## Explicit non-goals

This task does not add HTML/JS/CSS, frontend rename controls, Tauri UX or new
permissions, provider/MCP/Process Plugin HostExplicit support, generic
filesystem rename, live Windows certification, release preparation, or
v0.25 publication.
