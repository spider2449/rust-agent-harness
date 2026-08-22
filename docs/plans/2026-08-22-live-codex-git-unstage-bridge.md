# Live Codex `host.git.unstage` bridge validation

## Scope

Add one opt-in `rah-runtime-codex` example that validates the committed,
host-owned `host.git.unstage` capability through the restricted generic Codex
dynamic-tool bridge. It adds no Git capability and does not alter the tool's
authority model.

## Boundaries

- Register exactly `host.git.unstage`; the bridge allowlist contains only
  `PermissionLevel::Execute`.
- The model-visible schema is the existing empty object schema. Model input can
  provide no repository, path, pathspec, Git option, executable, argv, cwd,
  environment, or timeout.
- The trusted host supplies an absolute canonical native `git.exe`, an owned
  temporary repository, fixed `tracked.txt` target, fixed argv, cwd, and Git
  environment. Reparse-point executables and PATH discovery are rejected.
- Codex-owned shell, file-change, and MCP execution remain disabled; only the
  generic dynamic-tool bridge is enabled.

## Fixture and assertions

1. Commit initial `tracked.txt`, `unrelated-staged.txt`, and
   `unrelated-worktree.txt` content in an owned temporary repository.
2. Set `tracked.txt` to version A and stage it, then set it to version B without
   staging it. Stage an unrelated change and make a distinct unrelated
   worktree-only change.
3. Capture HEAD, refs, target HEAD-tree/index/worktree observations, unrelated
   index/worktree observations, the full index, canonical repository identity,
   and porcelain status. The target must be `MM` before execution.
4. Prompt Codex to call the sole tool once with `{}` and then return exactly
   `RAH_GIT_UNSTAGE_OK`.
5. Assert one requested/started/finished tool lifecycle, one fixed native
   `git restore --staged --source=HEAD` mutation, no retry or replay, a
   `Completed` terminal event, and verified-success ToolOutput.
6. Assert that the target index exactly becomes its pre-observed HEAD entry,
   target worktree bytes stay version B, all unrelated observations remain
   unchanged, and HEAD, refs, identity, and all non-target index entries do not
   change. Remove the owned fixture afterward.

## Validation

Run the normal deterministic workspace gates plus `git diff --check`, then run
the example using Codex CLI 0.148.0 with explicit absolute
`RAH_CODEX_EXECUTABLE` and `RAH_GIT_UNSTAGE_EXECUTABLE`. The live example is
intentionally excluded from the offline workspace test suite.
