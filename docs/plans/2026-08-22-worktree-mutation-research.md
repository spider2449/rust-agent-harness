# Worktree-mutation authority research plan

Status: Completed research; no implementation authorized
Date: 2026-08-22

## Scope

Evaluate the next repository authority layer after the verified index-only
`host.git.stage` and `host.git.unstage` milestones. This task changes
documentation only. It must not implement a restore, commit, ADR, public API,
or production policy.

## Method

1. Inspect ADR 0009, ADR 0010, the current private index-mutation policy, and
   prior v0.3 mutation research.
2. Compare staying index-only, a single host-owned `restore --worktree`, commit,
   and smaller mutations across Git and host-authority state planes.
3. Verify Git restore, attributes/conversion/filter, sparse-checkout, and hook
   semantics against official Git documentation.
4. Define the narrowest possible future argv/schema, postconditions, and
   destructive recovery requirements without implementing them.
5. Decide release scope and whether the new authority requires an ADR.

## Findings and acceptance criteria

- Worktree byte replacement/removal is a new destructive authority class; it
  is not covered by the existing index-only capability decision.
- A future narrow operation could retain the empty `{}` schema and fixed argv:
  `git --literal-pathspecs restore --worktree --source=HEAD -- <host target>`.
- It needs host-owned destructive authorization, a durable bounded preimage,
  stale-authorization refusal, recoverable result semantics, no retry/replay,
  and no automatic rollback.
- The initial worktree scope would reject staged-plus-unstaged targets,
  conversions/filters, sparse checkout, submodules, links/reparse points, and
  unsupported filesystem identities.
- ADR 0011 is required before implementation, but is deliberately not created
  by this research task.
- v0.3 should release after its verified index-only capabilities rather than
  adding worktree or history mutation.

## Deliverables

- `docs/RAH_V0.3_WORKTREE_MUTATION_RESEARCH.md`
- This plan record.
