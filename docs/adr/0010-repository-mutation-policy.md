# ADR 0010 — Repository mutation policy

Status: Accepted

## Context

ADR 0009 constrains construction and supervision of an Execute process, but a
successful process exit does not prove that only an authorized repository effect
occurred. `docs/RAH_V0.3_MUTATING_EXECUTE_RESEARCH.md` recommends a deterministic
fixture before any Git mutation capability.

## Decision

1. `PermissionLevel::Execute` remains broad execution authority.
2. Intentional repository mutation additionally requires a private, host-owned
   `RepositoryMutationPolicy`.
3. A model `ToolCall` is a request, never mutation authorization.
4. The trusted host selects and validates repository identity.
5. Mutation targets are host-owned symbolic identities.
6. The model supplies no paths, Git pathspecs, executable, argv, cwd,
   environment, or timeout.
7. Mutations serialize per repository identity.
8. A mutation lease begins before pre-state capture and ends after post-state
   verification and audit construction.
9. Authority is limited to explicit targets and effects.
10. Pre-state is captured before execution and post-state after execution.
11. Postconditions prove the authorized effect, reject unauthorized effects,
    and revalidate repository identity.
12. Process exit alone is insufficient authorization evidence.
13. Mutation is not assumed atomic, and RAH promises no rollback.
14. Timeout, cancellation, disconnect, crash, or a lost response after spawn
    can leave uncertain side effects; uncertain mutation is never replayed.
15. Existing bridge exact-once handling remains the duplicate-request boundary.
16. Host audit data is host-owned and excludes unnecessary paths and secrets
    from model output.
17. The initial prototype is a deterministic repository-owned fixture, not
    Git. Git add/commit, refs/history, submodules/worktrees, arbitrary paths,
    network Git, and file-content authoring are deferred.
18. No architecture-defining public RAH contract changes are authorized.

## Consequences

`rah-tools` owns the private policy because it combines repository state
semantics with an existing capability-specific Execute policy. The fixture has
an empty input schema and a fixed `fixture-marker` target, so no model data can
be parsed as a path. Its bounded full-root snapshot detects additions,
deletions, and unapproved changes in the test fixture scope. The in-process
lease serializes only RAH-owned concurrent mutation; it cannot prevent an
external process, so unexpected observations fail closed.
