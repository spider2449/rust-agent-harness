# Task 096: Windows `repo.edit-files` Codex Live Gate

## Scope

Validate the certified Windows `codex-cli 0.149.0` path using one isolated
temporary Git repository. The live fixture uses the authoritative
`TrustedStaticProfile -> rah_cli::profile_composition::compose -> fresh
ToolRegistry -> CodexRuntime::connect_tool_bridge` path and exposes only
`repo.edit-files` with Execute permission.

## Assertions

- One bridged `repo.edit-files` call replaces exact preimages in `b.txt` then
  `a.txt` request order, while the host reports effects in lexical `a.txt`,
  `b.txt` order.
- The structured output is `ok` with two `committed_verified` effects.
- Both tracked worktree files have exact postimages; the raw index, HEAD, refs,
  and baseline sentinel remain unchanged and Git reports only the two unstaged
  file modifications.
- The turn terminates with `Completed`; no approval, other tool, retry, or
  replay is accepted. The structural marker is emitted only after shutdown and
  all host assertions succeed.

## Non-goals

This is not a new compatibility matrix, Trusted Profile redesign, bridge
special-case, or Codex schema investigation. It adds no dependency and does
not alter the certified baseline, profile version, permissions, or ADRs.
