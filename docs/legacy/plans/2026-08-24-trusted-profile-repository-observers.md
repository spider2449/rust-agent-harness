# Task 063: trusted-profile repository observer composition

Status: Implemented
Date: 2026-08-24

## Profile contract

Task 063 reuses the existing `profile_version = 1` closed `capabilities[]`
schema. It introduces no `repository_observers` top-level section and no new
resource category. The following capability identities each use the existing
symbolic `executable` and `repository` bindings:

```json
{
  "name": "repo.status",
  "enabled": true,
  "permission": "execute",
  "executable": "git",
  "repository": "workspace"
}
```

The same shape applies to `repo.file-info`, `repo.diff`, and
`repo.diff-staged`. Each is a closed capability: `workspace`, `max_bytes`, and
`cwd_resource` are rejected, as are arbitrary Git arguments, revisions,
pathspecs, environment, and observer options. The symbolic Git and repository
resources may also be shared with `repo.patch`.

`PermissionLevel::Execute` is the existing outer subprocess gate. It does not
grant generic Git authority: each observer retains its fixed executable,
repository identity, read-only command family, fixed environment/config,
bounds, and exclusive lease. This is distinct from `repo.patch` and its private
worktree mutation policy.

## Deferred composition

Static loading records each enabled observer as a closed host-only
`RepositoryObserverProfile` containing only its canonical capability identity
and symbolic executable/repository identifiers. It checks the symbolic
resources' existence and kind but does not construct an observer, execute Git,
inspect repository state, or mutate files. Inventory therefore reports
`configured` and `registered = false`.

The existing real CLI effective composer resolves those symbols after deferred
`repo.patch` construction and before MCP and Process Plugin admission. It then
constructs the existing observer tools and registers their canonical names in a
fresh `ToolRegistry`:

- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`

No observer execute path is called during composition. Successful registrations
are generalized into the existing effective inventory update, which reports
`validated` and `registered = true`. Inventory remains redacted to capability
name, Execute permission, symbolic resource IDs, and state; it excludes host
paths, repository state, output, environment, and observer internals.

## Composition, cleanup, and coverage

Composition still fails closed for duplicate registry names and construction
errors. Observer construction owns no persistent provider child. Existing late
MCP/Process Plugin failure cleanup remains responsible for reaping already
started providers, while no partial registry is returned.

Deterministic tests cover closed static bindings, all four duplicate names,
invalid resource/field/permission cases, redacted static and effective CLI
inventory, non-mutation during validation, canonical Execute definitions, and
mixed `repo.patch` + observer + MCP + Process Plugin composition. Tests only
compose tools; they do not invoke observer execution.

## ADR status

No ADR changed or was added. The existing trusted-profile authority boundary
continues to govern this composition-only work.

## Suggested next task

Task 064 — deterministic Generic Tool Bridge verification for
trusted-profile-composed repository observers, without live Codex.
