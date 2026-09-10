# ADR 0025: HostExplicit Reviewed File-Deletion Boundary

Status: Accepted

Date: 2026-09-10

## Context

ADR 0017 accepts the separate host-owned authority for deleting one clean,
HEAD-tracked repository file. ADR 0021 accepts the generic HostExplicit
coordinator, currentness, D2, ticket, and provenance boundary. Neither
decision by itself accepts a reviewed human Desktop route for the existing
repo.delete-file capability.

Task 280 independently reviewed the capability-specific contract for that
route and concluded:

    A — REVIEWED DELETION HOSTEXPLICIT CONTRACT READY FOR ADR 0025

The reviewed contract is deliberately narrower than the ordinary deletion
Tool. It provides complete human review of a bounded text preimage while
preserving the existing deletion authority, Tool schema, result vocabulary,
and ToolRegistry dispatch path.

This ADR is an architecture decision only. Acceptance does not implement the
route, change frontend behavior, enable production execution, run live
certification, or complete the v0.24 release.

## Decision

### 1. Reviewed workflow and authority relationship

Accept exactly this future reviewed-human workflow:

    closed typed human {path}
     -> zero-effect deletion Prepare
     -> complete bounded backend-derived destructive review
     -> opaque process-local single-use ticket
     -> deletion-specific currentness revalidation
     -> D2
     -> reviewed Commit authorization invalidation
     -> HostExplicit Started
     -> authorized_tool_dispatch
     -> current ToolRegistry
     -> existing repo.delete-file
     -> ADR 0017 RepositoryFileDeletionPolicy
     -> strict result classification / descriptive refresh

ADR 0017 remains the sole underlying deletion mutation authority. ADR 0025
adds no filesystem deletion authority. ADR 0025 accepts only the reviewed
HostExplicit human workflow boundary. ADR 0021 remains the generic
HostExplicit coordinator/currentness/D2/ticket/provenance boundary.

The reviewed route must reach the existing Tool through:

    authorized_tool_dispatch
     -> current ToolRegistry
     -> repo.delete-file

Desktop and frontend must not directly invoke native deletion. No parallel
deletion implementation, filesystem delete shortcut, or alternate registry is
accepted.

The following distinctions are normative:

- model request is not authorization;
- human confirmation is not underlying mutation authority;
- frontend presentation is not authority;
- HostExplicit eligibility is not underlying deletion authority;
- PermissionLevel::Execute is not deletion authority;
- Tool registration is not deletion authority;
- Effective Authority visibility is not deletion authority;
- Trusted Profile metadata is not deletion authority;
- provider metadata is not deletion authority; and
- public Tool name, prefix, or category is not deletion authority.

Acceptance does not mean implementation, production enablement, live
certification, or v0.24 release completion.

### 2. Underlying deletion boundary and reviewed-route narrowing

ADR 0017 remains unchanged and authoritative for the ordinary
repo.delete-file Tool and RepositoryFileDeletionPolicy. Its ordinary Tool
surface permits a preimage up to 1 MiB and may include binary content.

The reviewed HostExplicit route is a capability-specific subset:

    ordinary ADR 0017 Tool authority:
    - up to 1 MiB
    - binary content may be accepted by the ordinary Tool

    reviewed HostExplicit deletion route:
    - complete human-reviewable strict UTF-8 only
    - NUL rejected
    - raw preimage 0 through 65536 bytes inclusive

This is a reviewed-route restriction, not a change to ADR 0017, the ordinary
Tool surface, or the existing Tool schema. The reviewed route must not be
widened to cover the whole ordinary Tool surface.

The effect remains exactly one existing regular repository file removed from
the worktree. The resulting Git meaning is one unstaged deletion. The route
does not Stage, Unstage, Commit, mutate the index, mutate HEAD, mutate a
branch or ref, mutate history, rename, move, restore, clean up, or delete
another path.

### 3. Closed human Prepare request

The human Prepare request is exactly:

    {
      "path": "src/module.rs"
    }

The typed DTO is closed. Unknown fields, missing fields, wrong types,
non-object input, NUL, and a request beyond the serialized bound fail closed
and issue no ticket.

The human must not supply:

- expected SHA-256;
- expected byte length;
- repository root;
- native path;
- parent path;
- Git executable;
- target or parent identities;
- HEAD, tree, blob, index, ref, branch, or raw-index identities;
- permission, effect, or authority fields;
- ToolName;
- raw ToolInput or raw ToolOutput;
- provider, model, runtime, profile, or connection selection;
- retry, replay, delete-again, recovery, restore, or rollback controls;
- Stage, Unstage, or Commit instructions;
- activity ID; or
- a ticket at Prepare time.

The host captures the exact preimage and derives the existing Tool-required
expected_file_sha256 and expected_file_byte_length. It reconstructs the
unchanged canonical three-field repo.delete-file Tool input. The existing
Tool schema is unchanged.

### 4. Exact reviewed-route bounds

All bounds are inclusive and are measured at the specified representation
boundary:

| Value | Bound |
| --- | --- |
| Human serialized request | 8192 bytes maximum |
| Logical path | 1 through 1024 UTF-8 bytes |
| Eligible raw preimage | 0 through 65536 bytes |
| Content | strict UTF-8; NUL rejected |
| Complete serialized review | 262144 bytes maximum |
| Opaque prepared representation | 524288 bytes maximum |

The logical path uses the existing safe repository-relative path grammar.
There is no truncation, implicit encoding conversion, Unicode normalization,
newline conversion, BOM insertion or removal, or hidden expansion. If the
complete required review or private retained state cannot fit its bound,
Prepare fails before ticket issuance.

### 5. Complete destructive review

Prepare constructs a complete backend-owned, display-only review. The complete
source must be shown; a preview is not sufficient. Authorization must not be
based on truncation, ellipses, collapsed hidden mutation-relevant content,
digest-only content, incomplete snippets, or binary/hex/base64 substitution
as the v1 default.

The complete source representation is deterministic and ASCII-safe:

- CR is escaped;
- LF is escaped;
- TAB is escaped;
- NUL is rejected;
- spaces, including trailing spaces, are made explicit;
- slash and quote escaping is used where required;
- C0 controls and DEL use fixed escapes;
- bidi, zero-width, and other format characters use fixed code-point escapes;
  and
- all non-ASCII scalars use deterministic fixed code-point escapes.

The escaped representation is display-only. It must not modify the exact
bytes retained for Tool input. The review must explicitly communicate, as
applicable, empty content, BOM presence, CR/LF/CRLF facts, final-newline
state, tabs, trailing spaces, controls, bidi characters, zero-width
characters, and format characters.

The direct review must communicate at least:

- operation = repo.delete-file;
- target_count = 1;
- validated logical path;
- clean current-HEAD-tracked stage-0 state;
- file mode 100644 or 100755;
- intent to permanently remove one existing regular file;
- complete escaped preimage;
- host-derived byte length;
- host-derived SHA-256 of the exact bytes;
- BOM, newline, and content facts;
- HEAD/blob relationship;
- index relationship;
- expected effect;
- post-delete Git meaning;
- non-effects; and
- destructive warnings.

The review must state:

    effect:
    remove one existing worktree file

    resulting Git meaning:
    one unstaged deletion

It must expressly state that the operation is not Stage, Unstage, Commit,
index mutation, HEAD mutation, ref/history mutation, rename, move, restore,
cleanup, or deletion of another path.

Warnings must include:

- permanent worktree removal;
- no Trash or Recycle Bin guarantee;
- no backup;
- no restore;
- no rollback;
- no retry or replay;
- uncertainty may require manual inspection; and
- timeout, cancellation, disconnect, or possible post-effect failure does not
  prove rollback.

The representation is complete within the 262144-byte serialized review
bound. If it cannot fit, Prepare fails with no usable confirmation ticket.

### 6. Exact eligible source and repository state

The reviewed route retains all ADR 0017 target requirements and narrows them
with the strict-UTF-8 and size rules. The eligible target is:

- exactly one existing regular repository file;
- current HEAD-tracked;
- represented by one normal stage-0 index entry;
- equal in worktree bytes to the current HEAD blob;
- equal in index state to the HEAD tree entry;
- mode 100644 or 100755;
- of exact protected target identity;
- of link count exactly one;
- in a supported ordinary repository state;
- strict UTF-8;
- free of NUL; and
- no larger than 65536 raw bytes.

Unrelated dirty paths may remain only where ADR 0017 already permits them.

Prepare must reject before ticket issuance, when applicable:

- missing target;
- directory or special target;
- dirty target;
- staged target change or deletion;
- untracked or ignored target;
- intent-to-add;
- conflicted or unmerged entry;
- submodule or gitlink;
- nested repository;
- .git or repository metadata;
- unsupported sparse or skip-worktree state;
- bare repository;
- linked worktree;
- detached or unborn HEAD;
- malformed or alternate index;
- merge, rebase, cherry-pick, revert, sequencer, bisect, or other
  unsupported repository state;
- symlink;
- junction;
- reparse point;
- hard-link ambiguity;
- unsafe target or parent alias;
- case-equivalent ambiguity;
- unsafe path or containment observation;
- binary or non-UTF-8 content;
- NUL content;
- content larger than 65536 bytes; and
- complete-review or prepared-state overflow.

The route does not claim race-free TOCTOU behavior or exclusion of external
editors, antivirus, Git, synchronization software, or privileged actors.

### 7. Zero-effect Prepare

Before Confirm, Prepare must perform exactly:

    0 Tool executions
    0 native delete attempts
    0 filesystem mutation
    0 index mutation
    0 HEAD/ref/history mutation
    0 Stage
    0 Unstage
    0 Commit
    0 model/runtime execution
    0 MCP execution
    0 Process Plugin execution
    0 temporary repository mutation artifact
    0 durable authority persistence

Bounded host reads, hashing, Git observation, identity inspection,
deterministic escaping, and bounded in-memory retention are allowed. Prepare
must not call Tool execution, ToolRegistry execution, DeleteFileW,
remove_file, or a replacement, cleanup, or recovery path.

Prepare failure issues no ticket. Prepare does not invalidate reviewed Commit
authorization and does not create an effectful HostActivity state.

### 8. Private preparation binding

The future implementation must retain an opaque, bounded, host-owned,
process-local in-memory preparation sufficient to reconstruct and revalidate
the existing Tool request. It must bind at least:

- logical path;
- private canonical/native path as required;
- exact source bytes;
- source SHA-256;
- source byte length;
- file mode;
- target FileIdentity;
- link count;
- repository-root identity;
- Git executable identity;
- .git identity;
- attached branch;
- HEAD OID;
- HEAD tree/blob entry;
- normal stage-0 index entry;
- protected raw index or implementation-proven equivalent;
- refs fingerprint required by ADR 0017;
- relevant repository, runtime, profile, model, connection, and composition
  generations;
- ToolRegistry and composition identity;
- exact ToolDefinition;
- current allowed permission membership and policy identity;
- Effective Authority identity where used by current HostExplicit currentness;
- capability-specific preparer identity;
- deterministic review identity; and
- canonical reconstructed ToolInput.

This private state may contain native paths, source bytes, raw index bytes, and
identity values only as bounded in-memory state required for this one
operation. It is not durable authority, is never accepted back from the
frontend, and is never placed in generic activity, logs, URLs, persistence,
telemetry, or provider metadata. If the complete prepared representation
cannot fit 524288 bytes, Prepare fails with no ticket.

### 9. Ticket contract

The ticket follows the existing HostExplicit pattern:

- RAH-generated;
- opaque;
- capability-specific;
- process-local;
- in-memory;
- single-use;
- currentness and exact-preparation bound;
- nonpersistent;
- nonresumable; and
- not itself filesystem authority, permission, or a durable capability.

The exact expiry boundary is:

    elapsed < 300 seconds: valid
    elapsed >= 300 seconds: expired

Exactly five minutes is expired. There is no persistence, resume, automatic
replacement ticket, or automatic reprepare.

Confirm receives exactly:

    {
      "ticketId": "..."
    }

Cancel receives the ticket only. Neither accepts path, hash, length, Tool
JSON, repository, permission, provider, model, authority, or review fields.
Wrong, malformed, duplicate, expired, and activity-ID tickets must never
execute.

The authority ticket ID and generic activity ID are independently generated
and distinct. An activity ID cannot Confirm or Cancel. A ticket cannot be
accepted under an alternate activity or invocation key.

### 10. Confirm ordering and revalidation

Confirm must consume the single-use ticket before asynchronous work and use
this exact ordering:

    consume ticket
     -> current connected composition check
     -> repository/runtime/generation/preparer checks
     -> deletion-specific shared revalidation
     -> exact ToolDefinition and permission membership check
     -> D2 authorize_tool_dispatch
     -> invalidate reviewed Commit authorization
     -> emit HostExplicit Started
     -> authorized_tool_dispatch
     -> current ToolRegistry
     -> existing repo.delete-file
     -> strict result classification
     -> descriptive refresh

D2 must happen before Started and before any mutation can begin. There is no
post-effect D2. The route never directly calls a Tool or native delete from
Desktop, builds a parallel registry, or uses a generic JSON console.

Immediately before D2 and the effect, revalidation must prove the same:

- selected repository;
- repository root, Git executable, and .git identities;
- capability-specific preparer;
- logical path;
- exact source bytes, hash, and length;
- target identity and link count;
- file mode;
- ordinary, no-link, no-reparse, and containment state;
- HEAD, tree, blob, and attached branch;
- exact stage-0 index and protected index state;
- refs fingerprint;
- supported repository state;
- review identity;
- canonical ToolInput;
- exact ToolDefinition;
- current permission membership;
- registry, composition, and Effective Authority identities; and
- relevant generation tuple.

Any material drift fails before a native attempt. There is no automatic
refresh of expected source, replacement ticket, or execution against newer
state. A fresh explicit Prepare is a new authorization, not replay.

### 11. Reviewed Commit interaction

Prepare does not invalidate reviewed Commit authorization. Cancel before
Started does not invalidate it. Stale, currentness, or D2 rejection before
Started does not imply a filesystem effect and does not invalidate Commit
authorization merely because a ticket was consumed.

Immediately before effectful HostExplicit Started, the backend invalidates
repository-bound reviewed Commit authorization. This invalidation occurs
before the owned dispatch task begins and therefore also covers post-Started
dispatch or runtime uncertainty.

Descriptive refresh may show the resulting unstaged deletion, but it must not
fabricate replacement Commit authorization, Stage the deletion, Unstage it,
Commit it, restore it, or rerun deletion. No automatic Stage or Commit exists
in this route.

### 12. Existing Tool status contract

ADR 0025 does not invent a Tool result vocabulary. It preserves exactly the
five existing statuses:

    deleted_verified
    known_no_effect
    invalid_input
    precondition_failed
    uncertain

The current Tool output contract remains one JSON content item with the exact
expected fields for the status:

| Status | Required output shape | is_error |
| --- | --- | --- |
| deleted_verified | status, uncertain=false, logical relative path | false |
| known_no_effect | status, uncertain=false, logical relative path | true |
| invalid_input | status, uncertain=false | true |
| precondition_failed | status, uncertain=false, logical relative path | true |
| uncertain | status, uncertain=true, logical relative path only where the underlying branch includes it | true |

The HostExplicit layer may strictly validate these outputs but may not
redefine them. It requires exactly one JSON content item, exact keys and
types, status-specific uncertain values, and the existing optional-path
behavior. A malformed or contradictory ToolOutput is never upgraded to
success or known no effect.

The logical relative path is never a native path. The HostExplicit generic
result is status-only and must not forward raw ToolOutput.

### 13. Stronger reviewed-route proof rules

The route-side classifier strengthens proof requirements without amending ADR
0017 or the Tool schema.

#### deleted_verified

deleted_verified requires:

- the one native delete attempt may have occurred;
- the exact Tool output is structurally valid;
- specifically confirmed NotFound absence for the exact target;
- no same-name or case-equivalent replacement;
- no reparse replacement;
- protected root, parent, and Git state remain valid;
- the index is unchanged;
- HEAD, tree, blob, attached branch, and refs remain unchanged; and
- no contradictory observation exists.

A generic metadata error does not prove absence.

#### known_no_effect

known_no_effect requires:

- exact structurally valid Tool output;
- fresh independent proof that the exact original target identity remains;
- the same link count;
- the same exact source bytes, hash, and length;
- the same HEAD/blob/index relationship; and
- the same protected Git fingerprint.

Native failure alone does not prove no effect.

#### uncertain

uncertain is required whenever adequate proof for either terminal effect class
is missing. The route must not guess from a native error, timeout, lost
response, process result, access failure, sharing failure, or generic
metadata error.

### 14. Windows DeleteFileW observation

The reviewed route accepts exactly one native deletion attempt. On Windows,
DeleteFileW success is the filesystem commit point but is not by itself proof
of verified absence.

The route uses one immediate post-attempt proof pass:

- no polling loop;
- no wait-until-disappears loop;
- no second delete;
- no replay; and
- no cleanup.

If DeleteFileW succeeds but the path remains visible because deletion is
pending, the result is uncertain. Same-name recreation, case-equivalent
recreation, reparse replacement, access/sharing/I/O observation failure,
delayed disappearance, lost result, or contradictory observation is uncertain
unless exact permitted proof independently establishes another class.

If DeleteFileW fails, known_no_effect requires exact identity-equal intact
proof of the original target, link count, bytes, hash, length, HEAD/blob
relationship, index relationship, and protected Git fingerprint. A native
failure alone is insufficient.

The route makes no race-free TOCTOU or universal open-handle behavior claim.

### 15. HostActivity mapping and refresh

The conservative HostActivity mapping is:

| Condition | HostActivity state | Generic result |
| --- | --- | --- |
| deleted_verified with exact output and independent proof | tool_completed | status-only deleted_verified |
| known_no_effect with exact output and intact proof | tool_error | status-only known_no_effect |
| invalid_input | tool_error | status-only invalid_input |
| precondition_failed | tool_error | status-only precondition_failed |
| uncertain | possible_effect_unknown | status-only uncertain |
| malformed or contradictory ToolOutput after Started | possible_effect_unknown | status-only uncertain |
| dispatch or runtime failure after Started | possible_effect_unknown | status-only uncertain |

Only deleted_verified has a generic success result. All other Tool statuses
are errors. A possible effect is never replayed.

Descriptive repository refresh is allowed after a terminal effect or result
when current. It must not create authority, upgrade uncertainty, issue a
replacement ticket, Stage, Commit, restore, or rerun deletion. A later human
may deliberately begin a fresh Prepare after a proven non-effect or
precondition result; that is a new operation.

Pre-Started HostExplicit lifecycle rejection remains distinct from the five
Tool statuses.

### 16. Review privacy and provenance

The complete exact preimage, source-derived SHA-256, source-derived byte
length, and BOM/newline/content facts may appear in the direct review because
explicit human authorization requires them. This is direct-review-only data.

Generic surfaces must not contain actual values of:

- source or preimage text or bytes;
- source sentinel values;
- escaped-source sentinels;
- source-derived SHA-256;
- source-derived byte length;
- BOM, newline, or content facts;
- authority ticket;
- raw ToolInput;
- raw ToolOutput;
- native repository path;
- native target path;
- FileIdentity;
- raw index bytes;
- private Git, repository, or preparer identities; or
- the complete review object.

Generic Prepared activity carries no source-bearing review and no ticket.
Generic terminal activity carries only the safe status-level result. The
ticket and activity ID must remain distinct by value, not only by field name.

HostExplicit is source-distinct from model lifecycle. The route emits no
AgentEvent::ToolRequested, AgentEvent::ToolStarted, or
AgentEvent::ToolFinished, creates no fake model turn, injects no ToolOutput
into conversation, and causes no automatic model continuation.

### 17. Future typed frontend contract

ADR 0025 may define the future UI contract, but Task 281 does not implement
it. The future surface is:

    path input
     -> Prepare
     -> complete destructive review
     -> Confirm or Cancel

The frontend sends only the typed path Prepare request and ticket-only Confirm
or Cancel. It does not compute or submit SHA-256, length, FileIdentity, Git
state, permission, Tool name, result proof, currentness, authority, raw
Tool JSON, native paths, or recovery controls.

Binary, NUL-containing, oversized, or otherwise unreviewable source must fail
before a usable confirmation ticket. The UI must not recommend bypassing this
reviewed route with generic deletion.

### 18. Future eligibility and model/provider boundary

Task 281 acceptance does not alter current eligibility. The exact current
nine-name HostExplicit set remains:

    fs.read
    repo.file-info
    repo.status
    repo.diff
    repo.diff-staged
    repo.create-branch
    repo.patch
    repo.edit-files
    repo.create-file

Only a later implementation and independent validation accepted under ADR
0025 may add exactly:

    repo.delete-file

That would make ten names. Still ineligible are:

    repo.rename-file
    repo.create-directory
    repo.commit
    MCP Tools
    Process Plugin Tools
    fixture/diagnostic Tools
    unknown Tools

There is no wildcard, category, permission-derived, effect-class,
provider-derived, public-prefix, or frontend-derived rule.

ADR 0025 is human/host initiated only. It does not authorize model-created
deletion tickets, model-selected HostExplicit deletion, provider-selected
HostExplicit deletion, MCP HostExplicit, Process Plugin HostExplicit, or
network/provider metadata admission. A model-requested preparation boundary
would require a separate decision.

### 19. Deterministic implementation obligations

Future implementation evidence must include, at minimum:

- closed {path} DTO;
- host reconstruction of unchanged Tool input;
- zero-effect Prepare;
- complete deterministic strict-UTF-8 review;
- all exact bounds;
- binary, NUL, and oversize refusal;
- private preparation currentness binding;
- source, identity, and Git drift refusal;
- exact five-status parser;
- identity-equal known-no-effect proof;
- specifically confirmed-NotFound success proof;
- Windows pending, recreation, and observation-failure uncertainty;
- exactly one native attempt;
- no polling, replay, cleanup, or recovery;
- ticket expiry, single-use, and activity separation;
- D2 before Started;
- current ToolRegistry dispatch;
- reviewed Commit invalidation;
- generic value-level privacy; and
- no fake model lifecycle.

These tests and implementation obligations are not implemented by Task 281.

### 20. Future Windows certification obligation

Later production-path certification must use:

- a fresh disposable Windows repository;
- one clean tracked strict-UTF-8 regular file no larger than 64 KiB;
- connected-current Desktop;
- exact ten-name eligibility only after implementation;
- Prepare with 0 Tool and 0 native attempts;
- Confirm with exactly 1 Tool dispatch and 1 native DeleteFileW attempt;
- verified path absence;
- resulting unstaged deletion;
- unchanged index, HEAD, branch, refs, and unrelated protected state;
- no Stage or Commit;
- ticket/activity separation;
- duplicate and activity-ID rejection;
- reviewed Commit authorization invalidation;
- zero model lifecycle;
- MCP count 0;
- Process Plugin count 0;
- coordinator Idle; and
- a host-attested marker.

No live test is run in Task 281.

### 21. Explicit non-goals

ADR 0025 does not authorize or imply:

- generic fs.write;
- generic filesystem delete or unlink;
- recursive delete;
- directory deletion;
- wildcard or glob deletion;
- untracked or ignored cleanup;
- partial-file cleanup;
- rename or move;
- directory creation;
- copy, overwrite, replace, append, or arbitrary write;
- backup or restore;
- Trash or Recycle Bin guarantee;
- automatic Stage or Unstage;
- automatic Commit;
- HostExplicit repo.commit;
- model-selected HostExplicit;
- provider HostExplicit;
- MCP or Process Plugin HostExplicit;
- network MCP;
- ticket persistence or resume;
- authority serialization;
- retry or replay;
- delete-again;
- rollback;
- compensation;
- recovery journal;
- generic shell or process authority;
- race-free TOCTOU claim;
- OS sandbox claim;
- network-isolation claim; or
- Linux/macOS live-certification parity claim.

### 22. Relationships to accepted ADRs

The relationship is:

    ADR 0017:
    sole underlying file-deletion mutation authority

    ADR 0021:
    generic HostExplicit dispatch/currentness/D2/ticket/provenance boundary

    ADR 0022:
    reviewed existing-file patch boundary

    ADR 0023:
    reviewed multi-file edit boundary

    ADR 0024:
    reviewed new-file creation boundary

    ADR 0025:
    reviewed human file-deletion HostExplicit boundary

ADR 0025 is additive and narrower. It supersedes none of these ADRs and does
not modify ADR 0017, ADR 0021, ADR 0022, ADR 0023, ADR 0024, or any other
accepted ADR. It does not change the repo.delete-file Tool schema, ordinary
model/runtime behavior, frontend behavior, Cargo, dependencies, versions, or
release identity.

## Acceptance

This ADR accepts only the future reviewed human HostExplicit workflow defined
above. The underlying mutation remains solely authorized by ADR 0017 and the
generic HostExplicit boundary remains solely governed by ADR 0021.

Task 281 is documentation-only. It does not implement Rust, add
repo.delete-file to HostExplicit eligibility, modify frontend behavior,
create shared preparers or backend code, run destructive live certification,
or start Task 282.
