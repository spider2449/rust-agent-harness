# `host.git.unstage` implementation plan

## Scope

Implement one deterministic, host-preauthorized Git index mutation in
`rah-tools`. The public capability is `host.git.unstage`, requires
`PermissionLevel::Execute`, and accepts only `{}`. It does not register a
Codex bridge, restore worktree content, commit, or add a general Git API.

## Implementation

1. Reuse the private single-target Git mutation policy already used by
   `host.git.stage`, retaining its canonical executable/repository/target
   identity checks, per-repository lease, fixed Git environment, state capture,
   and no-replay result semantics.
2. Add `GitUnstageTool::new(git_executable, repository_root,
   symbolic_target, target_path)` and bind the sole mutation to exactly:
   `git --literal-pathspecs restore --staged --source=HEAD -- <target>`.
3. Capture the fixed target's `HEAD` tree entry before execution. Accept an
   unstaging change only when the post-index target entry equals that entry,
   while the full unrelated index, worktree snapshot, `HEAD`, refs, repository,
   and executable observations remain unchanged.
4. Add deterministic local-repository tests for changed and no-op outcomes,
   staged-plus-unstaged preservation, validation/revalidation, lease
   serialization, unexpected index changes, timeout/lost-result uncertainty,
   and abort-without-replay behavior.

## Acceptance criteria

- The model can provide neither a path nor any Git process option.
- Normal execution runs the one fixed native Git mutation exactly once.
- A target worktree remains byte-identical, including when it has unstaged
  changes in addition to staged changes.
- Timeouts, cancellation/abort, and lost results never cause automatic retry or
  replay; incomplete proof is reported as `uncertain`.
