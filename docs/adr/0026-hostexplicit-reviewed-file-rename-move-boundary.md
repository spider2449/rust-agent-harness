# ADR 0026 — HostExplicit Reviewed File Rename/Move Boundary

Status: Accepted

Date: 2026-09-11

## Context

ADR 0018 accepts `RepositoryFileRenamePolicy` as the separate underlying
authority for moving exactly one clean, HEAD-tracked repository file. ADR 0021
accepts the generic HostExplicit coordinator, currentness, D2, ticket, and
provenance boundary. Neither decision by itself accepts a human-reviewed
HostExplicit route for `repo.rename-file`.

Task 297 independently re-read the ordinary implementation at the exact
checkpoint `2715a784a4e9c1b6dbc3612093f561039251e6b0`, whose direct parent is
`1bdbd5d86ef33c72eafedb8d526511b9b007c612`. The ordinary ADR 0018 gate found
no new material defect. The current implementation and focused deterministic
evidence cover the required ordinary boundary, including canonical root,
supported directory-form `.git` identity, Git executable identity, source and
destination Git admission, source/index/HEAD equality, complete ordinary
ancestry and nested-repository rejection, supported same-volume handling,
parent and file identity, Windows alias/case handling, one native no-replace
attempt, exact no-effect proof, independent post-effect proof, and
conservative `uncertain` classification. The implementation remains
check-and-revalidate best effort; it does not claim race-free handle-bound
semantics.

The ordinary authority is a prerequisite, not the reviewed workflow. A
reviewed route needs a narrower human input and review contract, private
host-derived preparation, an opaque ticket, capability-specific currentness,
independent result proof, and privacy rules. It must not create a second
filesystem rename authority or let Desktop bypass the current ToolRegistry.

## Decision

Accept exactly the following future human-reviewed HostExplicit workflow:

```text
human typed {source_path, destination_path}
 -> zero-effect host Prepare
 -> complete bounded host-derived review
 -> opaque process-local single-use ticket
 -> Confirm with ticket only
 -> coordinator/currentness/generation validation
 -> exact Tool and permission/D2 validation
 -> reviewed Commit authorization invalidation
 -> HostExplicit invocation
 -> authorized_tool_dispatch
 -> current ToolRegistry
 -> existing repo.rename-file
 -> ADR 0018 RepositoryFileRenamePolicy
 -> independent reviewed post-effect proof
 -> status-only activity and descriptive refresh
```

ADR 0018 remains the sole underlying rename/move authority. ADR 0021 remains
the generic HostExplicit coordinator, currentness, D2, ticket, and provenance
boundary. ADR 0026 adds only this capability-specific reviewed human route.
It does not add a filesystem primitive, amend the ordinary Tool schema, or
create a parallel rename implementation.

The following are not rename authority:

- model output or provider metadata;
- human confirmation or frontend state;
- an activity ID or review text;
- `PermissionLevel::Execute`;
- Tool registration, Tool definitions, or Effective Authority display;
- Trusted Profile metadata;
- MCP, Process Plugin, or arbitrary provider metadata; or
- any native path, filesystem identity, hash, index state, or repository
  identity supplied by the frontend.

Acceptance is an architecture decision only. It does not implement the
preparer, Desktop route, frontend surface, Tauri permissions, eligibility
allowlist, or live certification.

### 1. Human input and reviewed subset

The typed human Prepare request is exactly:

```json
{
  "source_path": "src/old.rs",
  "destination_path": "src/new.rs"
}
```

The request is closed and bounded. Unknown fields, missing fields, wrong
types, malformed logical paths, NUL, and serialized input beyond the route
bound fail closed without issuing a ticket. The human supplies no:

- source hash, byte length, content, or mode;
- filesystem or repository identity;
- Git executable, native absolute path, currentness, or generation proof;
- HEAD, branch, ref, tree, blob, index, or parent state;
- Tool definition, Tool input/output, permission, policy, or authority
  object;
- expected result status or post-effect proof;
- overwrite, retry, replay, rollback, cleanup, or recovery control; or
- activity ID, ticket ID, provider, model, runtime, profile, or connection.

The reviewed subset is intentionally narrower than ordinary ADR 0018:

- one existing regular file only;
- one same-directory rename or same-repository cross-directory move;
- an already existing destination parent;
- strict UTF-8, NUL-free source bytes;
- source raw bytes no larger than 65,536 bytes;
- complete deterministic escaped source representation, without preview,
  truncation, ellipsis, hidden regions, or digest-only authorization;
- complete serialized review no larger than 262,144 bytes; and
- bounded retained private preparation no larger than 524,288 bytes.

The complete review presents, at minimum:

- source repository-relative path;
- destination repository-relative path;
- source byte length;
- host-derived source SHA-256 or equivalent bounded immutable identifier;
- complete escaped source content and explicit format facts where required by
  the bounded review representation;
- the operation as one tracked clean file move;
- the expected unstaged Git consequence; and
- the explicit non-effects below.

The review must state that the operation does not:

- rewrite content;
- stage or unstage;
- commit;
- mutate a branch, ref, or history;
- create a directory;
- overwrite or replace a destination;
- rewrite imports or references; or
- automatically roll back, retry, replay, clean up, or compensate.

The ordinary Tool remains capable of its accepted ADR 0018 input range,
including its existing binary-capable ordinary route. The reviewed route's
strict UTF-8 and complete-review bounds are a HostExplicit narrowing and do
not change ordinary model/runtime behavior.

### 2. Source and destination eligibility

The reviewed source must be one safe repository-relative logical path to an
existing ordinary regular file beneath the selected canonical repository. It
must be cleanly tracked by current `HEAD`, represented by exactly one normal
stage-0 index entry whose mode and object agree with the `HEAD` tree entry,
and whose worktree bytes exactly equal the `HEAD` blob. The supported mode is
`100644` or `100755`, with worktree/index mode drift rejected. The source
identity and link count are host-captured, and link count must be one.

The source and every parent must be reached through ordinary, identity-bound
ancestry. Symlinks, junctions, reparse points, mount-like redirection,
nested repositories, metadata paths, unsafe aliases, special objects,
submodules, gitlinks, sparse or skip-worktree ambiguity, detached or unborn
HEAD, linked worktrees, malformed or alternate indexes, and active merge,
rebase, cherry-pick, revert, sequencer, or bisect states are rejected.

The reviewed destination must be one distinct safe logical repository-relative
path in the same selected canonical repository. Its existing parent chain
must remain ordinary and identity-bound. The destination must be absent from:

- the worktree, including files, directories, links, special entries, and
  alias-equivalent objects;
- the current `HEAD` tree;
- every current index stage, including intent-to-add and tracked-but-missing
  states; and
- the current Git ignore/exclude result for this reviewed route.

An ignored-but-absent destination is rejected by the reviewed route even when
ordinary ADR 0018 can technically permit it. This is a reviewed-route
narrowing that keeps the expected unstaged destination visible and auditable.

Windows case-only or otherwise equivalent renames are rejected. Trailing-dot
or trailing-space aliases, DOS device names, Unicode normalization ambiguity,
UNC/verbatim/device forms, reparse traversal, source aliases, overwrite,
replace, swap, cross-repository movement, cross-volume movement, and missing
parent directories are rejected. No temporary-name workaround is authorized.

### 3. Zero-effect Prepare and private evidence

Prepare performs bounded host-owned observation only. It performs zero Tool
executions, zero ToolRegistry dispatches, zero native rename attempts, zero
Git mutation, zero index/HEAD/ref mutation, zero Stage/Unstage/Commit, and
zero model, MCP, or Process Plugin execution. It may read, hash, parse,
inspect identities, and construct the direct review.

The retained process-local preparation must remain private and retain enough
evidence to revalidate the exact prepared operation. It includes, as
applicable:

- selected canonical repository/root identity, supported `.git` form and
  identity, and Git executable identity;
- exact source and destination logical paths and private native paths;
- source identity, link count, bytes, SHA-256, byte length, and mode;
- source and destination parent identities and safe ancestry evidence;
- source `HEAD` tree/blob and exact normal stage-0 index entry;
- destination worktree, `HEAD`, all-index-stage, intent-to-add, and ignored
  absence observations;
- same-volume/filesystem evidence and alias rules;
- raw index bytes or a collision-resistant equivalent fingerprint;
- `HEAD`, attached branch, protected refs, and supported Git-state markers;
- repository, runtime, profile, model, connection, registry, composition,
  and generation identities;
- the exact current ordinary ToolDefinition and permission membership;
- the retained capability-specific preparer identity;
- the host-derived canonical `repo.rename-file` ToolInput; and
- complete review identity and size accounting.

This evidence is in-memory only. It is never accepted back from the frontend,
serialized into generic activity, persisted, placed in URLs or telemetry, or
used for restart/resume. Preparation failure issues no ticket and does not
invalidate an existing reviewed Commit authorization.

### 4. Ticket, Confirm, and currentness

The ticket is RAH-generated, opaque, process-local, in-memory,
capability-specific, short-lived, single-use, currentness-bound,
nonpersistent, and nonresumable. Its validity boundary is:

```text
elapsed < 300 seconds: valid
elapsed >= 300 seconds: expired
```

Confirm and Cancel accept exactly a ticket identifier. They do not accept
paths, hashes, bytes, identities, Tool JSON, permissions, repository state,
review fields, or authority objects. The ticket ID is distinct from the
activity ID; an activity ID can never authorize Confirm or Cancel. Malformed,
wrong, expired, cancelled, duplicate, or activity-ID values execute nothing.

Before the effectful boundary, Confirm consumes the single-use ticket and
performs, in order:

1. connected-current coordinator and busy-state checks;
2. repository, runtime, profile, model, connection, and generation checks;
3. retained preparer identity and capability-specific revalidation;
4. exact ToolDefinition, composition, registry, Effective Authority, and
   explicit permission-membership checks;
5. D2 authorization before `Started` and before any possible effect;
6. invalidation of the prepared reviewed Commit authorization;
7. HostExplicit `Started` publication;
8. exactly one `authorized_tool_dispatch` through the current ToolRegistry;
9. strict result parsing and independent reviewed post-effect proof; and
10. status-only terminal activity and descriptive repository refresh.

Revalidation rejects drift in source bytes/hash/length or identity, either
parent or its ancestry, destination absence or ignored state, Git absence
proof, source index/HEAD equality, `HEAD`, branch, refs, active Git state,
repository/root/`.git`/Git identities, same-volume evidence, current
ToolDefinition or permission membership, composition/registry/currentness,
review identity, or canonical ToolInput. Hash and length equality alone are
never sufficient because a same-byte replacement can have a different
filesystem identity.

Desktop never directly performs filesystem rename. It never constructs raw
Tool JSON, native paths, or a second registry.

### 5. Reviewed Commit authorization interaction

Prepare, Cancel, and rejection before `Started` have no mutation effect and
do not revoke a prepared reviewed Commit authorization. Immediately before
effectful HostExplicit `Started`, the coordinator invalidates that
repository-bound reviewed Commit authorization. This conservative ordering
also covers dispatch failure, lost results, and uncertain outcomes after
`Started`, because a worktree effect cannot be ruled out.

A verified rename is an unstaged worktree mutation. It does not Stage,
Unstage, Commit, restore, or create replacement Commit authorization. A fresh
human operation is required after a proven no-effect or precondition result.

### 6. Existing Tool effect and result contract

The route dispatches only the host-derived, unchanged ordinary ToolInput:

```json
{
  "source_path": "src/old.rs",
  "destination_path": "src/new.rs",
  "expected_source_file_sha256": "<host-derived lowercase SHA-256>",
  "expected_source_file_byte_length": 123
}
```

ADR 0026 does not redefine the five existing producer statuses or their
single-JSON-content output shapes:

| Status | Required JSON content | `is_error` |
| --- | --- | --- |
| `renamed_verified` | `status`, `uncertain:false`, logical destination `path` | false |
| `known_no_effect` | `status`, `uncertain:false` | true |
| `invalid_input` | `status`, `uncertain:false` | true |
| `precondition_failed` | `status`, `uncertain:false` | true |
| `uncertain` | `status`, `uncertain:true` | true |

Unknown fields, missing fields, wrong types, contradictory values, extra
content items, or any other ToolOutput mismatch are not success. After
`Started`, malformed, lost, or contradictory output is conservatively
reviewed as `uncertain`.

The underlying effect remains exactly one host-selected native no-replace
rename attempt. Windows uses the existing `MoveFileExW` no-replace form;
Linux uses the existing `renameat2` `RENAME_NOREPLACE` form; unsupported
platforms fail closed. There is no `git mv`, copy-delete, shell/process move,
generic filesystem rename, temporary-name sequence, retry, replay, reverse
rename, cleanup rename, compensation, or rollback.

### 7. Independent reviewed proof

`renamed_verified` is not established by the Tool status alone. The reviewed
coordinator/preparer independently proves all of the following:

- the prepared source path is specifically absent;
- the prepared destination exists as the expected ordinary file;
- destination bytes exactly equal the prepared source bytes, including length
  and host-derived digest;
- destination identity corresponds to the protected source identity and
  supported link-count proof where the platform supports that observation;
- source and destination parents remain the expected identities and ordinary
  ancestry remains valid;
- selected repository root, supported `.git`, and Git executable remain
  valid/current;
- nested-repository, mount-like, reparse, alias, and case constraints remain
  valid;
- `HEAD`, attached branch, protected refs, raw index, source/index state, and
  all protected Git state remain unchanged; and
- the ToolOutput exactly matches the current producer contract above.

The proof makes no universal durability, atomicity, or identity claim beyond
what the supported platform can independently observe.

### 8. Known no effect and uncertainty

`known_no_effect` is accepted only when fresh independent host evidence proves
the exact prepared source preimage still exists with the expected identity,
link count, bytes, hash, length, mode, parents, repository/Git state, index,
and currentness, while the prepared destination is specifically absent. A
native error, a Tool status, or a failure to observe the destination is not
enough. A generic metadata error is not proof of absence.

If an effect may have occurred and neither exact reviewed success nor exact
prepared no-effect state can be independently proven, the result is
`uncertain`. Timeout, cancellation, disconnect, crash, native error, lost
result, contradictory state, or observer failure never implies no effect or
rollback. Uncertain results are never retried, replayed, reversed, cleaned
up, compensated, or automatically recovered.

### 9. Privacy, activity, and provider boundary

The direct human review may contain the complete bounded source review and
host-derived source facts needed for explicit authorization. Generic activity
may retain only bounded sanitized operation, status, and provenance data
consistent with the existing HostExplicit policy. It must not contain source
bytes or text, source-derived hash or length, native paths, filesystem
identities, raw index or Git state, Tool input/output, private aliases,
authority objects, or the ticket. Tickets are never serialized into generic
activity or persistence.

The route is source-distinct from model lifecycle. It emits no fake model Tool
request, Tool start, Tool finish, conversation message, ToolOutput injection,
or automatic model continuation. MCP, Process Plugin, provider-selected, and
model-selected HostExplicit rename are not accepted by this ADR.

### 10. Eligibility and nonclaims

Acceptance records that `repo.rename-file` is approved to become
HostExplicit-eligible in a later implementation and validation task. Task 297
does not change the shipped eligibility set, which remains exactly:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
repo.create-file
repo.delete-file
```

`repo.rename-file` remains ineligible until its capability-specific preparer,
backend, deterministic proof, privacy checks, Desktop route, and separately
authorized live certification are complete. This ADR does not enable
`repo.commit`, `repo.create-directory`, MCP Tools, Process Plugin Tools, or
arbitrary provider Tools.

This ADR does not imply:

- generic filesystem rename or arbitrary path movement;
- directory or recursive movement;
- wildcard/glob selection, overwrite, replace, swap, or parent creation;
- cross-repository or cross-volume movement;
- content editing, import/reference rewriting, or Git refactoring;
- Stage, Unstage, Commit, Git history/ref, shell, or generic process
  authority;
- durable ticket persistence, restart resume, or authority serialization;
- retry, replay, compensation, rollback, or recovery journaling;
- race-free TOCTOU, cross-process exclusion, OS sandboxing, or network
  isolation;
- universal native atomicity, durability, or file-identity preservation; or
- Linux/macOS production live parity from deterministic or Windows evidence.

## Relationships

- ADR 0018 remains the sole `repo.rename-file` filesystem mutation authority.
- ADR 0021 remains the generic HostExplicit dispatch/currentness/D2/ticket/
  provenance boundary.
- ADR 0022 remains the reviewed existing-file patch boundary.
- ADR 0023 remains the reviewed multi-file authoring boundary.
- ADR 0024 remains the reviewed new-file authoring boundary.
- ADR 0025 remains the reviewed file-deletion boundary.

ADR 0026 supersedes none of these ADRs. It does not change the ordinary
`repo.rename-file` schema, ordinary model/runtime behavior, current
HostExplicit eligibility, frontend authority, Trusted Profile semantics,
Cargo dependencies, or release identity.

## Acceptance

This ADR accepts only the future reviewed human HostExplicit file rename/move
workflow defined above. The accepted underlying effect remains solely owned by
ADR 0018 and must be reached through the current ToolRegistry and
`authorized_tool_dispatch`. Implementation, Desktop/frontend wiring, live
certification, and eligibility changes require separate authorized tasks.
