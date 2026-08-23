# Task 059: repository observer foundation and `repo.file-info`

Date: 2026-08-23
Status: Complete

## Scope

Implemented only the first fixed-command repository observer, `repo.file-info`,
and its crate-private shared observer envelope. `repo.status`, both diff
observers, trusted-profile composition, Generic Tool Bridge work, mutation
authority, permissions, and ADRs remain out of scope.

## Implementation

- Added a private `RepositoryObserver` that owns native executable and
  repository identity revalidation, a fixed canonical cwd, the Task 058 empty
  child environment, 5-second aggregate timeout, bounded process capture, and
  the existing exclusive per-repository RAH lease.
- It allows only the four Task 058 `repo.file-info` command shapes:
  `ls-files --stage -v -z --full-name --no-abbrev`, `rev-parse --verify -q
  HEAD`, `ls-tree -z -l HEAD`, and porcelain-v2 NUL status. The path, when a
  command accepts one, is appended as the single `HostArgumentPolicy::Text`
  value after `--`; no generic Git argv API exists.
- Added public host construction of `RepositoryFileInfoTool` only. Its closed
  request is `{"path":"<UTF-8 logical repository path>"}`. Paths are at most
  1 KiB and reject absolute/traversal/`.git`/backslash/colon/NUL forms.
- Normalizes HEAD/unborn, index stages, intent-to-add, assume-unchanged,
  skip-worktree, conflicts, sparse states, worktree kind/presence, mode and
  executable semantics. Success is always `status:"ok"` and
  `consistency:"best_effort"`.
- Returned parser-path infrastructure uses tagged UTF-8 or base64 values;
  selectable input remains UTF-8 only. No file content is returned. Present
  non-link regular files up to 1 MiB receive byte length and SHA-256 digest;
  binary data is not rejected just for being binary.

## Hardening and evidence

The child process starts from `env_clear` and receives exactly the Task 058 Git
configuration, optional-lock, and terminal-prompt variables. This prevents
inherited PATH/HOME/pager/proxy/credential/external-diff configuration. The
fixed calls do not invoke external diff or textconv helpers. Repository-local
configuration remains Git semantic input. The implementation asserts no
intentional fixture changes to HEAD, index, target content, or root entries;
it does not claim zero incidental filesystem writes by Git or the host.

Windows-native deterministic tests cover CRLF, Unicode, clean/unstaged/staged,
untracked/missing, unborn, index-only, conflict, skip-worktree/sparse omitted,
input rejection, malformed records, and local hostile `diff.external` helper
non-execution. Unix-only executable, symlink, and case-sensitive fixtures are
cfg-gated for Ubuntu CI.

## Architecture impact

No new dependency edges, public authority object, permission level, or ADR.
`PermissionLevel::Execute` remains the outer gate; the private fixed policy
contains actual authority.
