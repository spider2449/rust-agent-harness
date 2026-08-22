# Live Codex `host.git.stage` bridge validation

## Scope

Add one opt-in example in `rah-runtime-codex` that validates the already
committed host-owned `host.git.stage` capability through the restricted Codex
dynamic-tool bridge. This plan adds no new capability and does not alter the
authority model.

## Boundaries

- The example registers exactly `host.git.stage` and allows only
  `PermissionLevel::Execute`.
- The model-visible input schema is the tool's existing empty object schema.
- The trusted host supplies the absolute canonical native Git executable,
  temporary repository, authorized `tracked.txt` target, cwd, argv, and Git
  environment. The model supplies none of those values.
- Codex shell, file, and MCP execution remain unavailable; only the generic
  dynamic-tool bridge is enabled.

## Fixture and assertions

1. Create an owned temporary repository using the configured native Git.
2. Commit `tracked.txt`, `unrelated-staged.txt`, and `unrelated-worktree.txt`
   with a repository-local setup identity.
3. Make `tracked.txt` dirty, stage an independent change to
   `unrelated-staged.txt`, and make an unstaged independent change to
   `unrelated-worktree.txt`.
4. Record HEAD, refs, root identity, target bytes/index entry, unrelated
   bytes/index entry, complete index, and porcelain status.
5. Prompt Codex to make exactly one call with `{}` and reply exactly
   `RAH_GIT_STAGE_OK`.
6. Assert bridge event counts, completed terminal event, verified successful
   structured `ToolOutput`, fixed stage argv, no retry/replay, and one stage
   mutation.
7. Assert the target alone changed in the index and all unrelated/HEAD/ref/root
   boundaries remain intact, then remove the owned fixture.

## Validation

Run the standard deterministic workspace gates and `git diff --check`, then
run the example with Codex CLI `0.148.0`, an explicit `RAH_CODEX_EXECUTABLE`,
and an explicit absolute `RAH_GIT_STAGE_EXECUTABLE`. The live example remains
excluded from the normal offline test suite.
