# Task 298 — Reviewed Rename Backend Preparation / Proof Foundation

## Scope

Implement only the `rah-tools` backend foundation for the ADR 0026 reviewed
human rename/move route:

- closed typed `{source_path, destination_path}` preparation input;
- zero-effect host observation and complete bounded review;
- private opaque preparation evidence and deterministic review identity;
- capability-specific revalidation;
- strict current ordinary `repo.rename-file` output classification;
- independent reviewed success, exact known-no-effect, and uncertain proof;
- deterministic filesystem/Git fixture tests.

No coordinator, ticket, Desktop, frontend, Tauri, eligibility, provider,
live-certification, or release work is part of this task.

## ADR 0026 mapping

The implementation maps the accepted boundary as follows:

```text
typed human request
 -> zero-effect RepositoryRenameFilePreparer::prepare
 -> complete RepositoryRenameFileReview
 -> private RepositoryRenameFilePreparation
 -> capability-specific revalidate/proof helpers
```

The future effect path remains unchanged and is not implemented here:

```text
Confirm -> coordinator -> D2 -> authorized_tool_dispatch
 -> current ToolRegistry -> existing repo.rename-file -> ADR 0018 proof
```

The ordinary `repo.rename-file` schema and authority remain unchanged, and the
tool remains outside the current HostExplicit eligibility set.

## Zero-effect boundary

Prepare and revalidation perform no Tool execution, ToolRegistry dispatch,
native rename, filesystem mutation, Git mutation, index/HEAD/ref mutation,
Stage/Unstage/Commit, model execution, MCP execution, or Process Plugin
execution. They may read bounded filesystem and Git state, hash source bytes,
and build private evidence/review values.

## Bounds and review

- logical paths use the existing ordinary logical-path validation and 1024-byte
  path bound;
- serialized typed preparation input is at most 8192 bytes;
- reviewed source bytes are strict UTF-8, NUL-free, and at most 65536 bytes;
- complete escaped source content is retained and displayed without truncation;
- complete serialized review is at most 262144 bytes;
- retained private preparation representation is at most 524288 bytes.

The review binds source and destination paths, source length and SHA-256,
complete escaped source, source format/mode facts, expected unstaged Git
consequence, intended move effect, and explicit non-effects including no
content rewrite, staging, unstaging, commit, ref/history mutation, directory
creation, overwrite, import/reference rewrite, retry, replay, rollback, or
compensation.

## Retained private evidence

The opaque preparation retains the preparer identity, canonical repository/root,
`.git` and Git identities, private source/destination paths, source bytes and
identity, link count, parent identities, ancestry-validated preimage, HEAD and
index/Git fingerprints, destination ignore/absence observations, same-volume
admission, exact ordinary ToolDefinition and permission, host-derived ordinary
ToolInput, complete review, review identity, and size accounting. Its Debug
implementation is sanitized.

## Revalidation strategy

Revalidation reconstructs the canonical ordinary ToolInput and independently
recaptures the reviewed source/destination and repository state. It rejects
source byte/hash/length, identity, link count, mode, parent/ancestry,
repository/`.git`/Git, nested-boundary, destination absence/ignore,
HEAD/branch/ref/index/active-state, same-volume, ToolDefinition/permission,
review, or identity drift. Same-byte replacement is stale because FileIdentity
is part of the captured preimage.

## Proof strategy

The strict output classifier accepts only the current one-JSON-content
`repo.rename-file` producer shapes. `prove_result` never trusts status alone:

- success requires fresh source absence, destination identity/bytes/length/hash,
  ordinary ancestry, repository/Git identity, protected Git state, and exact
  success output;
- known-no-effect requires the exact protected source preimage and identity,
  current protected state, and specifically absent destination;
- all malformed, contradictory, ambiguous, observer-failure, or otherwise
  unproven states are uncertain.

No proof path retries, replays, reverses, cleans up, compensates, or rolls back.

## Deterministic coverage

The focused `rah-tools` integration suite covers valid same- and
cross-directory preparation, zero-effect state, host-derived ToolInput, closed
input rejection, source size/UTF-8/NUL/tracking/dirty/staged/mode/link checks,
destination parent/collision/HEAD/ignore/nested/symlink checks, complete
escaped review and review overflow, unchanged and drifted revalidation,
same-byte identity replacement, independent success/no-effect proof, malformed
output, and no-recovery behavior. Platform-specific filesystem cases are
compiled only where the platform provides the relevant primitive.

## Explicit non-goals

This task does not add HostExplicit eligibility or a HostInvocationKind, issue
or consume tickets, wire a coordinator, dispatch through `authorized_tool_dispatch`,
invalidate Commit authorization, add Desktop commands, change frontend or Tauri
files, modify providers or runtime behavior, perform live certification, or
prepare/publish a release.
