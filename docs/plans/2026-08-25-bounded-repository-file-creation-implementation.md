# Bounded Repository File Creation Implementation

## Scope

Task 085 adds only the `rah-tools` core primitive for `repo.create-file`.
Trusted-profile composition, Generic Tool Bridge integration, and live Codex
validation remain deferred.

## Implementation strategy

- Keep a private host-owned `RepositoryFileCreationPolicy` separate from the
  existing replacement policy and acquire the existing per-repository mutation
  lease before mutation-sensitive validation.
- Parse a closed request containing the existing symbolic repository resource,
  a slash-separated repository-relative path, and UTF-8 text content.
- Reject administrative, ignored, indexed, HEAD-present, sparse unsupported,
  submodule, link/reparse, and invalid namespace targets before the commit
  point. Revalidate those facts immediately before it.
- On Unix, walk existing parents with directory descriptors using `openat` and
  `O_NOFOLLOW`, then use `openat` with `O_CREAT | O_EXCL | O_NOFOLLOW` and mode
  `0o600` from the verified parent descriptor.
- On Windows, open each existing parent relative to the prior directory handle
  with `NtCreateFile` and `FILE_OPEN_REPARSE_POINT`, reject any reparse object,
  and create the final component relative to that pinned handle with
  `FILE_CREATE`. This ties creation to the verified parent identity rather than
  reusing a validated pathname.
- Treat successful native exclusive create as the commit point. Later failures
  never delete, retry, truncate, or replay the target.

## Fault seams and tests

Private test-only seams cover lease acquisition, initial validation, immediate
pre-create validation, post-create, mid-write, post-write, and final
verification. Tests cover Git/index/ref/sentinel invariants, target and parent
rejection, request/content bounds, deterministic target races, partial writes,
and an unobservable post-commit verification failure.

## Deferred work

Task 086 composes the primitive through trusted profiles and the unchanged
Generic Tool Bridge. Task 087 adds the separately authorized live Codex gate.
