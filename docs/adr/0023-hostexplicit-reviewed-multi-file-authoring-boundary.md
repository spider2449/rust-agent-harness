# ADR 0023 — HostExplicit Reviewed Multi-File Authoring Boundary

Status: Accepted

Date: 2026-09-08

## Context

ADR 0014 authorizes the existing bounded `repo.edit-files` multi-file
worktree mutation. ADR 0021 defines the general HostExplicit dispatch,
currentness, D2, coordinator, ticket, and provenance principles. ADR 0022
defines the reviewed HostExplicit workflow for the existing single-file
`repo.patch` capability.

Task 257 established that a separate capability-specific reviewed workflow for
`repo.edit-files` can be bounded and completely reviewed. Multi-file editing
differs materially from ADR 0022: the existing capability has deterministic
multiple native commit points and first-class `partial_effect` and `uncertain`
outcomes. It nevertheless reuses the same HostExplicit lifecycle and
authority-separation principles.

The trusted host remains the source of repository selection, capability
composition, permission policy, preimage identity, currentness, and execution
authority. Human input is a closed change request, not a Tool call, permission
grant, or authority-bearing set of bindings.

## Decision

### 1. Scope and authority relationship

This ADR authorizes a future capability-specific HostExplicit reviewed
workflow for exactly:

    repo.edit-files

Acceptance of this ADR does not change the current HostExplicit allowlist and
does not make `repo.edit-files` eligible. Actual eligibility requires later
implementation and validation tasks. Until those tasks pass, the current code
continues to reject `repo.edit-files` for HostExplicit execution.

No other Tool becomes eligible through this ADR. In particular, the following
remain explicitly excluded:

- `repo.create-file`;
- `repo.delete-file`;
- `repo.rename-file`;
- `repo.create-directory`;
- `repo.commit`;
- MCP Tools;
- Process Plugin Tools; and
- unknown Tools.

ADR 0014 remains authoritative for actual `repo.edit-files` multi-file
worktree mutation. ADR 0023 creates only the reviewed HostExplicit route to
that existing Tool; it does not create, replace, widen, or compose the
underlying worktree mutation authority.

### 2. Product capability

The future capability is:

> **HostExplicit Reviewed Multi-File Edit Authoring** — a connected-current
> Desktop human may prepare, review, and explicitly confirm one bounded
> change-set over existing clean tracked files through `repo.edit-files`.

One reviewed change-set may contain:

- 1–4 existing clean HEAD-tracked regular strict-UTF-8 files;
- 1–16 exact literal replacements per target; and
- at most 64 replacements total.

The change-set remains within all existing ADR 0014 request, file, and
postimage bounds and the additional reviewed-workflow bounds established by
Task 257:

- serialized Prepare request: at most 256 KiB;
- each path: at most 1024 UTF-8 bytes;
- each text value: at most 64 KiB;
- each file and postimage: at most 1 MiB;
- aggregate original/postimage material: at most 4 MiB;
- escaped changed material: at most 768 KiB across all targets;
- unchanged context: at most 32 KiB per target and 128 KiB total;
- complete serialized review: at most 1 MiB;
- stored pending review state: at most 2 MiB; and
- terminal activity/result metadata: at most 32 KiB, redacted.

This is one explicitly confirmed change-set, not four independently
authorized actions. It is not a transaction and has no cross-file atomicity
claim.

### 3. Closed human Prepare DTO

The only human mutation intent accepted by the future workflow is the closed
typed DTO:

    MultiFileEditPrepare {
        targets[] {
            path
            replacements[] {
                expectedOldText
                replacementText
            }
        }
    }

The human supplies only `targets`, `path`, `replacements`,
`expectedOldText`, and `replacementText`. Targets are bounded to 1–4. Each
target has 1–16 replacements, and every `expectedOldText` is nonempty while
`replacementText` may be empty for literal removal. Unknown fields fail
closed.

The human must not supply `ToolName`, arbitrary Tool JSON, hashes, byte
lengths, canonical or native paths, repository roots, Git commands or
parameters, provider identity, `PermissionLevel`, authority flags, ordering,
execution controls, or model metadata. The form provides no regex, glob,
fuzzy edit, unified diff, line/range mutation language, normalization, case
folding, or newline conversion. All literal matches are evaluated against
the same original snapshot.

The host translates the DTO into the existing closed `repo.edit-files`
`ToolInput` and `ToolCall`. The frontend never constructs trusted ToolInput.

### 4. Host-owned preparation

Prepare is backend-owned, non-effectful, and capability-specific. It derives
and binds the exact:

- selected repository identity and currentness;
- canonical repository-relative target identities;
- parent and target identity protections;
- clean HEAD/index tracked state;
- regular-file, link, reparse-point, hard-link, mode, and attribute admission;
- strict-UTF-8 bounded preimages, including BOM and newline bytes;
- complete preimage SHA-256 digests and byte lengths;
- exact literal match positions and duplicate/overlap/no-op checks;
- complete postimages, postimage SHA-256 digests, and byte lengths;
- deterministic host execution order;
- existing `repo.edit-files` ToolInput and ToolCall;
- expected complete ToolDefinition and current permission state;
- protected index, HEAD, ref, and repository observations;
- composition, registry, Effective Authority, and currentness generations; and
- coordinator, workflow, preparer, and repository bindings.

Prepare must produce:

- 0 Tool executions;
- 0 native replacements;
- 0 persistent target mutations;
- 0 index, HEAD, branch, ref, tag, or history mutations;
- 0 Stage, Unstage, or Commit operations;
- 0 model or runtime lifecycle events;
- 0 provider activation; and
- 0 persistent authorization or capability state.

Read-only repository and Git observation and bounded in-memory review
construction are permitted. Repository-adjacent temporary files must not be
created by Prepare merely to reuse the effectful Tool preflight
implementation. The future preparer retains the needed material in bounded
memory or host-private non-repository storage and leaves effectful Tool
temporary handling to the Tool path.

Prepare success is a review snapshot, not authority. A failed or oversized
Prepare issues no ticket.

### 5. Exact bounded review

The review is backend-derived and display-only. It must include every
mutation-relevant change in the complete reviewed change-set:

- every target path, target count, stable target identity, and target ordinal;
- total target and replacement counts;
- host-determined execution order, which is not human input order;
- replacement count for every target;
- every changed byte range in the original snapshot;
- complete escaped `expectedOldText` and `replacementText` for every range;
- explicit original-snapshot matching semantics;
- exact preimage and postimage SHA-256 and byte-length evidence;
- deterministic representation of significant whitespace, control
  characters, BOM, CR/LF, final-newline, bidi, zero-width, and format
  conditions;
- a complete mutation-relevant change representation for every target;
- the intended conditional worktree effect;
- protected non-effects; and
- an explicit warning that the operation is non-atomic.

Escaping is display-only and never normalizes or changes the bytes to be
written. No mutation-relevant changed content may be hidden, clipped,
ellipsized, redacted, or made confirmable behind a generic editor. Only
unchanged surrounding context may be omitted, and the review must say when it
was omitted.

The review carries forward all Task 257 bounds: complete changed material,
unchanged context, complete serialized review, pending review state, and
redacted terminal metadata must each remain within their respective limits.
If the complete changed material or complete review cannot fit or cannot be
understood within those bounds, Prepare returns `review_too_large` and issues
no ticket. The bound cannot be bypassed with a hidden source panel or generic
editor.

### 6. Authorization ticket

The authorization ticket is:

- RAH-generated;
- opaque and capability-specific;
- process-local and in-memory;
- nonserializable;
- single-use;
- bound to the exact change-set and repository/currentness; and
- valid for an inclusive five-minute TTL.

Expiry, cancellation, restart, disconnect, recomposition, or currentness
failure invalidates the ticket. The ticket is a lookup key, not repository
authority, permission, or mutation authority.

The ticket binds the exact:

- `repo.edit-files` capability name;
- canonical ToolInput and ToolCall;
- expected complete ToolDefinition and allowed permission state;
- complete review and review identity;
- target set, target identities, parent identities, and host order;
- preimage hashes/lengths and derived postimages/hashes/lengths;
- repository, `.git`, Git, index, HEAD, refs, and tracked-clean observations;
- selected repository and repository/workflow identity;
- composition, registry, Effective Authority, and preparer identity; and
- repository, model, profile, connection, coordinator, and currentness
  generations.

Confirm input is exactly:

    ticketId

Confirm must not accept paths, source content, hashes, postimages, Tool JSON,
permissions, repository data, provider data, model data, or authority data
from the frontend. The frontend must never reconstruct trusted ToolInput.

### 7. Confirm sequencing and exactly-once dispatch

The future sequence is normatively:

    ticket-only Confirm
    -> atomically reserve/consume ticket
    -> shared capability-specific revalidation
    -> D2 preflight
    -> HostExplicit Started
    -> authorized_tool_dispatch
    -> ToolRegistry
    -> existing repo.edit-files
    -> bounded result classification
    -> repository refresh

Ticket reservation and consumption occur before asynchronous revalidation.
An expired, stale, malformed, cancelled, or D2-rejected ticket is not
restored. A duplicate Confirm finds no pending ticket. One confirmed ticket
therefore permits a maximum of one Tool execution.

After execution can begin, the ticket can never become reusable because of a
timeout, disconnect, malformed result, partial effect, uncertain effect,
frontend retry, or duplicate Confirm. There is no retry or replay.

The first D2 preflight checks the current complete ToolDefinition, current
permission membership, current registry/composition, and host-owned inputs
before `Started`. Only after D2 succeeds may HostExplicit `Started` be
recorded. The owned execution then calls `authorized_tool_dispatch`, which
rechecks current definition and permission immediately before ordinary
`ToolRegistry` execution. No direct Tool call, raw registry shortcut,
generic ToolName-plus-JSON route, model lifecycle, or fabricated model event
is authorized.

### 8. Shared revalidation

Immediate fail-closed revalidation is required before Tool execution. It must
repeat all Task 257-bound assumptions for every target, including:

- selected repository, repository context, and connected-current state;
- repository, `.git`, fixed Git executable, and repository identity;
- target and parent identities;
- regular-file, link, reparse-point, hard-link, mode, and attribute admission;
- clean HEAD/index state, tracking, sparse/conflict state, and index entries;
- exact preimage hashes and byte lengths;
- exact literal replacement applicability and derived postimages;
- deterministic target set and host execution order;
- protected index, HEAD, refs, target entries, and other ADR 0014
  observations;
- expected complete ToolDefinition;
- current permission and Effective Authority membership;
- composition, registry, and preparer identity;
- model, profile, repository, connection, and currentness generations; and
- coordinator reservation and workflow currentness.

The host distinguishes conservatively among:

- stale authorization for ticket, repository, composition, generation, or
  workflow drift;
- invalid target for unsafe path, identity, file-type, link, or admission
  changes;
- precondition failure for changed bytes, applicability, postimage, or
  protected Git state; and
- D2/currentness rejection for definition, permission, registry, or dispatch
  admission drift.

All such pre-start failures produce zero Tool executions and no fresh
authorization. A changed target set or order is stale authorization unless
canonicalization itself is unsafe, in which case it is an invalid target.
An inability to prove a post-start inventory is uncertain, never a reason to
replay or continue.

### 9. Non-atomic result semantics

`repo.edit-files` is explicitly **NON-ATOMIC**. ADR 0014 remains authoritative
for its result classes:

- `ok`;
- `invalid_target`;
- `precondition_failed`;
- `failed_known_no_effect`;
- `partial_effect`; and
- `uncertain`.

HostExplicit must preserve these classes and must not collapse them into
generic success or failure.

For `ok`, report every target as verified in deterministic host order with
its exact expected postimage. For `invalid_target`, report the bounded
invalid-target reason and no target effect. For `precondition_failed`, report
the bounded failed precondition and no native mutation by this Confirm path.
For `failed_known_no_effect`, report the verified unchanged stop target and
later targets as not attempted where proven.

For `partial_effect`, report only the verified committed prefix, in host
order. Identify the remaining targets as not verified committed and classify
them only as `unchanged_verified` or `not_attempted` where proven. Do not
imply rollback, atomicity, or continuation.

For `uncertain`, state only what can still be proven. Never infer no effect
from timeout, cancellation, disconnect, a lost response, or an error. Do not
assert that an unclassified target is unchanged, committed, atomic, or
rolled back.

Automatic retry, replay, remaining-prefix continuation, rollback,
restore-preimage, Stage, Unstage, and Commit are prohibited. Repository
refresh is observation and reconciliation, not recovery.

### 10. Post-effect authorization invalidation

Terminal effectful outcomes invalidate the consumed edit ticket. If an
effectful execution reaches HostExplicit `Started`, repository-bound reviewed
authorization based on the old worktree/Git observations is conservatively
invalidated, including any older reviewed Commit authorization and any older
reviewed authoring authorization for the affected repository. This remains
true for successful, partially effectful, and uncertain outcomes; no
invalidated authorization is recreated.

After a terminal `ok`, `failed_known_no_effect`, `partial_effect`, or
`uncertain` result, repository refresh may produce a new descriptive review
state. It must use fresh observations and review identity. Refresh cannot
automatically authorize Commit, create a new edit ticket, or make the old
authorization valid. Stage, Unstage, and reviewed Commit remain separate
explicit human actions.

Fresh review is not authorization. The workflow does not create persistent or
resumable authoring authority, and it does not infer a repository-generation
change solely from worktree content if existing Desktop semantics do not
increment that generation.

### 11. Privacy and provenance

Full reviewed source/change material may exist only in bounded process-local
pending authorization state and in the backend-derived local review DTO
needed to show the human the exact change. It must not be persisted into
generic activity, conversation history, transcript persistence, logs,
telemetry, evidence bundles, or raw errors.

Those surfaces may retain only bounded redacted metadata: operation kind,
counts, relative or opaque target labels, order/count metadata, review
identity, result class, per-target redacted effect state, bounded failure
class, currentness identity, and boolean lifecycle counters. They must not
retain old text, replacement text, preimages, postimages, raw ToolInput,
ticket material, absolute/native paths, temporary names, command lines,
environment, credentials, provider output, or content-bearing backend
errors.

Confirm does not inject fake ToolOutput, fake model turns, chat continuation,
source-bearing activity, or runtime/model lifecycle events. HostExplicit
provenance remains explicit and distinct from model/runtime Tool execution.

### 12. Explicit non-authority statements

ADR 0023 does not authorize:

- generic filesystem write;
- generic repository editing;
- arbitrary ToolName or arbitrary Tool JSON execution;
- create, delete, rename, or directory operations;
- Stage, Unstage, Commit, or history mutation;
- shell or process execution;
- MCP or Process Plugin HostExplicit execution;
- provider self-authorization;
- model-selected authority;
- frontend authority;
- rollback, compensation, or restore-preimage;
- atomicity or transaction semantics;
- replay or continuation;
- persistence or resume;
- network authority;
- race-free TOCTOU guarantees;
- OS sandboxing; or
- network isolation.

`PermissionLevel::Execute`, Tool presence, Effective Authority visibility,
provider metadata, frontend confirmation, and human review do not create or
escalate the underlying repository authority.

### 13. Relationship to existing ADRs

ADR 0014 remains authoritative for `repo.edit-files` actual mutation,
admission, host-controlled ordering, native commit points, preconditions,
and `ok`, `invalid_target`, `precondition_failed`,
`failed_known_no_effect`, `partial_effect`, and `uncertain` semantics. ADR
0023 must not widen or replace `RepositoryMultiFileMutationPolicy`.

ADR 0021 remains authoritative for general HostExplicit dispatch,
connected-current requirements, D2, coordinator, ticket, currentness, and
provenance principles. ADR 0023 adds no generic dispatch or permission
authority.

ADR 0022 remains authoritative for the reviewed HostExplicit single-file
`repo.patch` workflow. ADR 0023 reuses its closed human input, host-derived
bindings, zero-effect Prepare, exact review, opaque ticket-only Confirm,
privacy, and no-replay principles but does not amend or widen its semantics.

ADR 0012 remains the underlying one-file `repo.patch` worktree-content
authority, and ADR 0016 remains separate reviewed index/history Commit
authority. Multi-file editing does not imply Stage or Commit.

No amendment to ADR 0014, ADR 0021, or ADR 0022 is made by this decision.

### 14. Future implementation and certification gate

ADR acceptance alone does not make `repo.edit-files` HostExplicit eligible.
Later implementation and validation must prove, at minimum:

Prepare:

- 0 Tool executions; and
- 0 native replacements.

Successful Confirm:

- at most 1 Tool execution;
- deterministic host target order; and
- exact expected postimages.

Staleness and currentness:

- zero execution on stale review;
- duplicate ticket cannot execute twice; and
- ticket-only Confirm cannot accept trusted bindings from the frontend.

Result classification:

- known no-effect;
- `partial_effect`; and
- `uncertain`.

Privacy:

- full change/source content is absent from generic persistence, activity,
  logs, and evidence surfaces.

Authority:

- only `repo.edit-files` is later added to HostExplicit eligibility;
- all previously ineligible authoring and external Tools remain rejected;
- model/runtime authority remains unchanged; and
- the frontend cannot bypass the ticket-only route.

Later deterministic tests must also prove complete review bounds, exact
preimage/postimage binding, all-target revalidation, D2-before-Started,
exactly-once dispatch, no retry/replay/continuation/rollback, conservative
partial and uncertain results, authorization invalidation, and protected
index/HEAD/ref state.

Windows connected-current live certification is a later gate. Task 258 makes
no Windows live certification claim and does not authorize implementation,
allowlist, permission, Trusted Profile, frontend, workflow, Tool, test,
dependency, or package-version changes.

## Consequences

RAH has a documented path for a future exact human review of one bounded
multi-file change-set while preserving the existing multi-file authority and
its non-atomic failure model. The cost is a capability-specific preparer,
complete bounded review, process-local ticket state, shared revalidation,
strict result presentation, and later deterministic and Windows evidence.

The current HostExplicit allowlist, repository mutation authority, model and
runtime authority, permissions, Trusted Profile, provider composition, and
release identity remain unchanged by accepting this ADR.
