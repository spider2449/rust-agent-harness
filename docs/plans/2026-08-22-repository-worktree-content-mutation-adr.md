# Task 047: Repository worktree content-mutation ADR research and draft

Status: Completed research and ADR draft; no implementation authorized
Date: 2026-08-22

## Scope

Research and define a new authority boundary for bounded repository worktree content mutation after RAH v0.4.0. This task changes documentation only. It does not modify Rust code, Cargo manifests, public APIs, trusted-profile schemas, tool registration, Codex bridges, or live examples.

## Method

1. Inspect ADR 0009 through ADR 0011, the v0.3 index/worktree research, the v0.5 authority roadmap, current rah-tools policy seams, and the read-oriented WorkspacePolicy boundary.
2. Compare an independent private RepositoryWorktreeMutationPolicy with broadening ADR 0010, generic filesystem write, and generic process authority.
3. Define one tracked-file literal replacement with complete-file digest/length and unique expected-old-text preconditions, including stale/ambiguous cases.
4. Research Windows path namespaces, links/reparse points, hard links, file identity, sharing, and replacement behavior; record supported limits rather than claiming race-free isolation or rollback.
5. Compare in-place modification with complete same-filesystem temporary-file replacement and record failure/cancellation/no-replay semantics.
6. Draft ADR 0012 as Proposed, leaving ADR 0010 and ADR 0011 unchanged.

## Decisions recorded

- New private host-side RepositoryWorktreeMutationPolicy is required; it is separate from RepositoryMutationPolicy, HostExecutionPolicy, WorkspacePolicy, ExternalToolPermissionPolicy, Git history/ref mutation, and network Git authority.
- Provisional repo.patch is one existing tracked stage-0/HEAD file and one exact literal expected_old_text -> replacement_text operation per call, guarded by full raw-file SHA-256 and byte-length preconditions.
- The initial version rejects untracked, staged, new, deleted, moved, binary, regex/glob/multi-file, .git, generic-write, Git index/history, shell, and network authority.
- Same-directory, same-filesystem complete-postimage temporary replacement is preferred to in-place writing. It is not a transaction or rollback promise.
- Strict UTF-8 with an exactly preserved optional leading BOM and no implicit newline normalization is recommended.
- Windows support is fail-closed for reparse points, hard links, path aliases, special namespaces, locks, and unsupported metadata; residual TOCTOU races remain an explicit non-guarantee.
- Uncertain effect is never replayed, and cancellation never implies rollback.

## Deliverables

- docs/RAH_V0.5_WORKTREE_MUTATION_RESEARCH.md
- docs/adr/0012-repository-worktree-content-mutation-authority.md
- This completed plan record.

## Validation

Run:

    git diff --check
    cargo check --workspace
    git status --short
    git diff --stat
    git diff

Confirm that only the three requested documentation files changed, with no Rust, Cargo, public API, profile-schema, tool-registration, Codex, or live example changes.

## Suggested next task

Task 048 should implement only the narrow deterministic foundation for private RepositoryWorktreeMutationPolicy and repo.patch in rah-tools, with owned temporary-repository adversarial tests. It must not yet compose the capability through trusted profiles or add a Codex bridge/live example.
