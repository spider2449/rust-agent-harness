# Task 048: Deterministic repository worktree mutation foundation

Status: Implemented
Date: 2026-08-22

## Scope completed

`rah-tools` now contains a private, host-owned
`RepositoryWorktreeMutationPolicy` behind the host-constructed public
`RepositoryWorktreePatchTool`. The only tool name is `repo.patch`; it is not
yet included in trusted-profile composition, Codex bridging, examples, MCP, or
Process Plugin integration.

The fixed input is one logical repository-relative slash-separated path, raw
whole-file SHA-256 and byte-length preconditions, one nonempty literal old
text, and replacement text. The policy rejects unknown fields, NUL, paths over
1 KiB, text fields over 64 KiB, serialized requests over 64 KiB, raw or
postimage files over 1 MiB, non-UTF-8/NUL content, and model-supplied BOM
characters. A single leading UTF-8 BOM is preserved as transport metadata;
matching excludes it. No LF/CRLF conversion or other normalization occurs.

Before the one replacement attempt, host-owned bounded Git observations require
the configured root to be the non-bare worktree root with a direct non-reparse
`.git` directory, a single regular HEAD entry, a matching single regular
stage-0 index entry, and normal (not sparse/skip-worktree) index flags. This
rejects untracked, staged, unmerged, linked-worktree, directory, submodule, and
gitlink targets. The Git executable, cwd, arguments, environment, and timeout
are host-owned through the existing trusted execution policy; no generic Git
or process authority is exposed to the model.

Root and path components reject aliases, links, reparse points, `.git`, parent
traversal, absolute/UNC/verbatim/device forms, backslashes, and colon/ADS
syntax. On Windows, root/parent/target identities are collected through native
handles using volume plus file index, and target hard-link counts are rejected.
The implementation also rejects read-only, compressed, and encrypted target
attributes. ACL and filter-driver behavior is not inferred: failed or
incomplete post-observation is conservatively reported as uncertain.

The complete postimage is built and flushed in a uniquely named exclusive
same-directory temporary file. Windows uses exactly one native Unicode
`MoveFileExW` replacement call with replacement and write-through flags; other
platforms use one same-filesystem rename call. There is no retry, rollback,
restore, or uncertain-effect replay. Known pre-commit failures clean the
temporary file where proven; known replacement failure requires proving the
preimage remains intact. Private evidence retains only target identity,
preimage/postimage hashes and lengths, and result classification, never content
or native paths.

## Internal result classes

- `Refused`: precondition/path/repository/temporary failure before a mutation
  attempt.
- `Success`: exact postimage plus repository and index/HEAD/ref observations
  verified after the native replacement.
- `KnownReplacementFailure`: replacement failed and the preimage was proven
  intact.
- `Uncertain`: replacement or post-observation could not prove the target
  state. It is never replayed automatically.

## Deterministic test coverage

Owned temporary Git repositories cover successful UTF-8 replacement,
BOM/CRLF preservation, wrong hash/length, missing/duplicated expected text,
malformed UTF-8, oversized postimage, untracked/staged/non-stage-0 targets,
directory/absolute/traversal/`.git`/ADS/namespace/alias paths, symlink and hard
link rejection where supported, injected stale-target failure with no attempt,
one replacement attempt, temporary cleanup after known failure, unchanged
unrelated files, and unchanged Git index.

Windows is the verification baseline for this task. The test suite passed with
the Windows native replacement and native identity path enabled. No Unix
runtime verification is claimed here.

## Deferred

ADR 0012 remains Proposed. Trusted-profile composition, Codex behavior, live
validation, generic writes/processes, Git index/history/ref mutation,
restore-worktree, plugins, MCP, and broader edit forms remain out of scope.

## Suggested next task

Task 049 should audit and harden this deterministic boundary with additional
race, locking, and replacement fixtures before any trusted-profile or Codex
integration.
