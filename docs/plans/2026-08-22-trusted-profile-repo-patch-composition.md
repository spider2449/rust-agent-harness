# Task 050: trusted-profile composition for hardened `repo.patch`

Status: Implemented
Date: 2026-08-22

## Scope

Task 050 composes the Task 049-hardened `repo.patch` capability through the
existing trusted static-profile and CLI effective-composer path. It adds no
Codex bridge or live validation, does not execute `repo.patch` in profile
tests, and does not change mutation semantics.

## Profile contract

The only enabled `repo.patch` capability shape is:

```json
{
  "name": "repo.patch",
  "enabled": true,
  "permission": "execute",
  "executable": "symbolic-git-resource",
  "repository": "symbolic-worktree-resource"
}
```

Both bindings are symbolic host resources from the existing `executables` and
`repositories` maps. The closed schema rejects raw roots, policy settings,
shell/argv/environment fields, and unrelated capability bindings. Static
loading validates source/schema/version and the symbolic reference shape, then
records the capability as `configured`; it does not construct the tool, run
Git, or inspect/mutate worktree content.

`rah profile validate-effective` uses the existing `rah-cli` effective composer
to resolve those symbols and call `RepositoryWorktreePatchTool::new`. Only that
already-hardened constructor creates its crate-private
`RepositoryWorktreeMutationPolicy`, with its fixed limits, canonical repository
identity, workspace/path confinement, host-owned Git observation policy, and
deterministic eligibility checks. The profile never owns or deserializes the
policy. It also does not replace or widen `WorkspacePolicy`; that remains the
separate boundary for capabilities that use it.

The tool remains `PermissionLevel::Execute`, matching its fixed host-owned Git
observation requirement. This is an outer runtime gate, not mutation authority:
`Execute != RepositoryWorktreeMutationPolicy`, and neither profile presence nor
a model request grants arbitrary worktree writes.

## Composition and lifecycle

Effective composition first copies ordinary static built-ins, then constructs
and registers canonical `repo.patch` through the normal `ToolRegistry`, then
admits MCP and Process Plugin providers in the existing order. Duplicate
registration remains fail-closed. `repo.patch` has no child provider or
persistent temporary ownership; construction is non-mutating. Existing failure
cleanup still reaps any already-staged provider, and no partial registry or
inventory is published.

The redacted inventory reports only `repo.patch`, `Execute`, symbolic resource
IDs, and configured/validated registration state. It excludes native paths,
contents, hashes, temporary paths, policy internals, credentials, and
environment.

## Deterministic coverage

Tests use the real static loader and `rah-cli` composer/CLI. They cover valid
symbolic construction, permission preservation, redacted inventory, static and
effective non-mutation, wrong/non-root resources, missing/unknown/wrong-type
resources, missing or invalid permission/binding, raw authority-field rejection,
duplicate capability rejection, mixed built-in/MCP/Plugin composition, and
provider cleanup after late failure. They assert registration and inventory only
and never execute `repo.patch`.

## ADR status

ADR 0012 remains **Proposed**. Profile composition does not accept the ADR,
broaden the policy, or authorize generic write/process/Git history/network
authority.

## Suggested next task

Task 051 — deterministic Generic Tool Bridge verification for
trusted-profile-composed `repo.patch`, including permission preservation and
single-execution/no-replay behavior, without a live Codex run.
