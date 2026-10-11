# `host.git.stage` implementation plan

## Scope

Implement one real, host-preauthorized Git index mutation in `rah-tools`.
The public capability is `host.git.stage`, has `PermissionLevel::Execute`, and
accepts only `{}`. It neither registers a live Codex bridge nor introduces a
generic Git abstraction.

## Existing boundaries used

- `HostExecutionPolicy` supplies canonical native executable validation, a
  cleared/fixed environment, exact argv, timeout supervision, and direct
  process execution.
- `RepositoryMutationPolicy` establishes the mutation lifecycle: a
  same-repository lease, pre-state capture, one execution, post-state capture,
  and a deterministic violation/uncertain result with no replay.
- `GitStatusTool` supplies the existing repository identity and deterministic
  Git environment conventions.
- The repository mutation fixture supplies the result semantics and lease /
  timeout test patterns, but remains a separate deterministic fixture.

## Steps

1. Extract only the private Git repository identity/environment helpers needed
   by both status and staging; retain their current behavior and public APIs.
2. Add `GitStageTool::new(git_executable, repository_root, symbolic_target,
   target_path)`. Trusted construction canonicalizes the repository and target,
   rejects links/directories/out-of-root targets, and binds the sole symbolic
   target to that one regular file.
3. Build private, exact `HostExecutionPolicy` commands for tracking checks and
   state observation. The sole mutating command is exactly
   `git --literal-pathspecs add -- <host-owned-relative-target>`.
4. Under the existing per-repository mutation lease, revalidate identities,
   capture HEAD, refs, worktree bytes/identity, and index bytes; verify the
   target is tracked; execute once; recapture and verify that only that target's
   index entry may differ while all other captured state remains equal.
5. Return bounded structured results distinguishing a verified changed stage,
   verified no-op, violation, known failure, and uncertain effect. Do not retry
   or replay after a timeout, abort, lost process result, or failed post-state.
6. Add deterministic temporary-local-repository integration tests using a
   locally discovered native Git executable and explicit per-command local
   author configuration. Cover the requested success, no-op, rejection,
   preservation, serialization, uncertain, and identity-revalidation cases.

## Acceptance criteria

- The model-visible schema is exactly an empty object.
- Capability execution performs direct native Git invocation with no shell or
  PATH lookup: `--literal-pathspecs add -- <exact target>`.
- No public architecture boundary changes; all Git state helpers are private to
  `rah-tools`.
- Postconditions fail closed for unapproved index/worktree/ref/root changes.
