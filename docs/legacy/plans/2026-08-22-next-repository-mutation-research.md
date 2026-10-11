# Next repository-mutation authority research plan

## Scope

Research and recommend the smallest next host-owned repository mutation after
the verified `host.git.stage` milestone. This is documentation-only work: do
not implement, register, or live-validate another capability.

## Method

1. Inspect ADR 0009, ADR 0010, the Git environment, and `GitStageTool`.
2. Compare a one-target unstage, worktree restore, commit, and a smaller
   index-only candidate against every state plane and policy layer.
3. Confirm current Git semantics and hook exposure with official Git docs.
4. Record the recommended capability, exact empty schema/fixed argv, required
   verification, and ADR decision.

## Deliverable

- Add `docs/RAH_V0.3_NEXT_REPOSITORY_MUTATION_RESEARCH.md`.
- Preserve public RAH boundaries; make no production-code changes.
- Explicitly defer generic Git/process execution, arbitrary paths/pathspecs,
  network Git, retries/replay, and commit authority.
