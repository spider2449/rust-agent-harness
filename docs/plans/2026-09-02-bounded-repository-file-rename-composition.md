# Task 172 — Bounded repository file rename composition

## Scope

Compose the Task 171 `repo.rename-file` capability through the existing trusted
host and Generic Codex Tool Bridge paths. Desktop integration, live Codex
validation, release work, and observability formatting changes remain deferred.

## Composition contract

The trusted profile accepts only the same closed symbolic executable and
repository resource binding used by the bounded repository capabilities. Static
profile loading records the binding but constructs no authority. The host must
separately construct and inject the opaque `RepositoryFileRenameAuthority`,
which remains bound to the selected repository resources. Missing or mismatched
authority fails closed and does not expose the tool.

The effective composer registers the ordinary `RepositoryFileRenameTool` in a
fresh `ToolRegistry`. No profile field creates rename authority, selects raw
paths, changes policy limits, or composes authority from other capabilities.

## Bridge proof

The public name is `repo.rename-file`; the Task 171 description, Execute
permission, and four-field closed schema are preserved. The deterministic test
observed a private alias from the actual bridge snapshot (currently
`rah_tool_0` for that one-tool fixture); this is test-local and not a stable API.
The dynamic description identifies the canonical public name and the request is
routed by generic alias lookup.

The deterministic cross-directory fixture proved one successful bridge call,
one native rename effect, unchanged bytes, unchanged index/HEAD/refs, no stage,
no commit, no replay, and exactly one `ToolRequested`, `ToolStarted`, and
`ToolFinished` event. Duplicate delivery reused the generic bridge response.
Stale source preimage and missing authority failed closed. Execute remains only
the outer dispatch permission; it does not construct the rename authority.

## Authority separation and remaining work

Rename remains separate from creation, deletion, worktree patch, index
mutation, commit authority, provider metadata, tool definitions, and profile
data alone. ADR 0011 and ADR 0018 are unchanged. Desktop integration and
Windows live Codex validation remain deferred.

Task 163's v0.13 live-evidence issue where `ToolContent::Json` can be recorded
as `null` was not changed by this task. The rename result is structured JSON
and the generic bridge response conversion preserves it. A small generic
observability-hardening task should precede eventual v0.14 live validation if
the live evidence helper still handles only text content.
