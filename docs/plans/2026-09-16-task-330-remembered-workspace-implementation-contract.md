# Task 330 — Remembered Workspace Persistence Implementation Contract

Date: 2026-09-16

Status: Accepted implementation contract for Task 331

This document turns ADR 0028 and the settled Task 329 research into a direct
implementation boundary. It is not a second research exercise and does not
authorize production implementation outside the slices named here.

## 1. Delivery boundary

Task 331 implements only the persistence foundation for a host-owned,
descriptive remembered-candidate catalog. The foundation is strictly below
repository authority:

```text
closed durable catalog
  -> inert in-memory descriptive records
  -> no repository member
  -> no active repository
  -> no executable repository composition
```

The implementation must use a dedicated host-owned `RememberedWorkspaceStore`
backed by `remembered-workspace.json` in the normal per-user application-owned
data directory. It must not extend desktop preferences, conversation
transcripts, repository files, provider profiles, or any general settings or
Tool store.

The store owns only descriptive catalog lifecycle: closed parsing, validation,
bounded load/save/delete, in-process catalog synchronization, atomic
replacement, corruption handling, and bounded storage errors. It does not own
admission, activation, repository identity, authority composition, provider
or runtime lifecycle, or frontend authority.

## 2. Data contract

Implement the closed v1 record exactly as follows:

```text
Root      = { version: u64, workspace: Workspace }
Workspace = { id: OpaqueId, label: Label, members: Member[],
              last_active_member_id?: OpaqueId }
Member    = { id: OpaqueId, label: Label, location_hint?: PathHint }
```

`Member.id` is a durable `RememberedCandidateId`, not a
`RepositoryMemberId`. Generate it with a host-owned bounded random strategy.
It must be opaque, persistent, descriptive only, and independent of path,
repository name, filesystem identity, Git identity, credential, or authority
handle. No repository API may accept it as a currentness, identity, or
activation value.

Required fields are `version`, `workspace.id`, `workspace.label`,
`workspace.members`, and each member's `id` and `label`. Optional fields are
`location_hint` and `last_active_member_id`. The latter must reference an
existing member when present and is only a presentation hint.

Unknown fields, duplicate keys, missing required fields, nulls, wrong types,
malformed UTF-8, trailing data, invalid control characters, invalid IDs,
invalid references, and invalid bounds fail closed. Schema version must be
exactly `1`; future versions are preserved and reported unsupported.

The initial bounds are fixed and independently testable:

| Value | Maximum |
| --- | ---: |
| Serialized final file | 256 KiB |
| Candidate records | 64 |
| Workspace/member label | 256 UTF-8 bytes each |
| Opaque ID | 64 ASCII bytes |
| Location hint | 4096 UTF-8 bytes |
| Schema nesting | Fixed closed-schema depth |

Every count and string must be bounded before allocation or publication. The
serialized byte limit is checked before parsing and before replacement.

`location_hint` is absent by default. If explicitly supplied by a human
catalog action, it is only a bounded platform-representable absolute/native
path string without NUL or control characters. Persistence does not
canonicalize, stat, open, inspect `.git`, derive identity, or test availability
of the hint. A bad or over-limit hint is rejected or omitted as descriptive
input; it never creates authority.

The record must not contain `RepositoryMemberId`, `workspace_epoch`, live
membership/admission/repository generations, filesystem or `.git` identity
proof, Git executable identity, `DesktopRepository`, `ToolRegistry`, Effective
Authority, permissions, D2, Commit authorization, Stage/Unstage action IDs,
index reservations, HostExplicit tickets/preparations, provider/runtime
handles, Codex runtime, connection currentness, executable conversation
context, credentials, Tool data, transcript data, or effect/replay/recovery
state.

## 3. Storage and atomicity contract

`RememberedWorkspaceStore` is the sole catalog storage owner. Its conceptual
location is the existing app-owned per-user data directory, with the dedicated
file `remembered-workspace.json`. It must not use a repository/workspace root,
neutral execution directory, provider profile directory, shared/network path,
`desktop-preferences.json`, or `conversation-transcript.sqlite3`.

The owner must preserve the normal app-data permission boundary and fail
closed on an unsupported link, Windows reparse/device form, or storage
redirect. No encryption-at-rest claim is made.

Serialize catalog writes inside the process. For another process or stale
instance, perform a bounded OS/file coordination attempt; if exclusive
coordination cannot be obtained, return `save_failed` and retain the existing
final file. Do not merge catalogs and do not assume last-writer-wins.

The successful write path is:

1. Validate the entire in-memory closed record and serialized byte bound.
2. Create a unique temporary file with the store's exact owned prefix in the
   same directory as the final file.
3. Write all bytes, flush/synchronize, and close the temporary file.
4. Parse/validate the staged bytes with the same closed v1 parser.
5. Perform one supported native same-directory replacement.
6. Return success only after replacement succeeds.

Create, write, flush, close, coordination, replacement, and cleanup failures
are bounded `save_failed` results. The old final file remains authoritative
until replacement succeeds. On a failed attempt, clean up only the temporary
file owned by that attempt. Startup cleanup may remove only stale temporary
files matching the exact store-owned naming rule. It must never sweep the
application-data directory.

Windows sharing/locking and replacement failures are normal explicit test
cases. The catalog synchronization owner must never serialize catalog writes
with model turns, Codex runtime, Stage/Unstage, Commit, ToolRegistry lifecycle,
provider connection, or repository switching, and must not introduce a global
authority lock.

## 4. Load, corruption, and migration contract

Load outcomes are closed:

| Input | In-memory result | File action |
| --- | --- | --- |
| Valid v1 | Bounded inert catalog | No rewrite until explicit mutation |
| Missing | Empty catalog | Do not create at startup |
| Oversized, malformed, truncated, wrong-type, duplicate, unknown-field, invalid-bound, or invalid-location | Empty catalog plus bounded `catalog_unavailable` | Optional same-directory `.corrupt` quarantine only after close |
| Future version | Empty catalog plus bounded `unsupported_version` | Preserve; do not quarantine, downgrade, or rewrite |
| Ownership/coordination failure | Empty catalog for this process plus bounded storage failure | No rewrite |

Corrupt data is never partially trusted and is never silently replaced by an
empty successful record. Best-effort quarantine must preserve recoverable bytes,
use a same-directory name governed by an exact store-owned rule, and leave the
source intact if the rename fails. Errors and diagnostics contain categories,
not raw storage paths, hints, identities, or authority data.

Schema v1 has no generic migration framework. A later schema requires a
separate bounded, loss-aware decision. A future migration must parse the old
closed version, validate a new closed record, preserve a recoverable source,
and replace atomically only after validation. V1 does not drop unknown fields,
guess values, merge other stores, or restore executable state.

## 5. Startup contract

Startup may load and parse the one dedicated bounded catalog and construct an
inert presentation-safe in-memory catalog. It must perform no candidate-path
filesystem operation. In particular, startup must not stat/open/canonicalize a
hint, resolve reparse targets, inspect `.git`, invoke Git, scan nested
repositories, probe removable/offline/network media, or validate provider or
runtime state.

Startup must create zero repository members, zero active repository, no
`DesktopRepository`, no repository ToolRegistry, no Effective Authority, no
Commit/Stage/Unstage/HostExplicit state, no provider/runtime/Codex binding,
and no executable conversation repository context. A valid
`last_active_member_id` can affect presentation focus only.

Automatic re-admission and automatic activation are both forbidden for v0.27.
An explicit fresh admission bridge is a later slice. It must use the existing
ADR 0027 admission pipeline from scratch and, only after success, create a new
process-local `RepositoryMemberId`, fresh admission/membership/currentness
generations, and an inert member. Explicit activation is a separate later
action and must compose only the selected repository.

## 6. Catalog operation contract

The storage foundation must support the descriptive primitives required by
later slices:

- load the complete bounded catalog or a bounded failure result;
- save a complete validated catalog transactionally;
- delete the durable catalog record or candidate record through the same
  atomic owner; and
- preserve ordering and optional last-active presentation metadata when a
  complete catalog is saved.

Later host catalog actions map to these primitives:

- remember/add creates a fresh durable candidate ID and may include a hint
  only after explicit human opt-in;
- update changes only label or hint descriptively;
- reorder changes only array presentation order; and
- delete removes only durable descriptive data.

No catalog primitive may delete filesystem content, mutate Git, remove a
process-local member, deactivate an active repository, revoke authority,
change ToolRegistry, alter Commit/Stage/Unstage/HostExplicit state, disconnect
a provider, or change Codex/runtime state. Durable candidate deletion and
process-local member removal are separate features and member removal is out
of scope for Task 331.

If repository admission later succeeds but catalog persistence fails, the
admission transaction remains valid under its existing process-local authority
contract. Persistence reports failure without fictional rollback, replay, or
authority refresh. A failed descriptive save is not reported as durable
success.

## 7. Production implementation slices

### Slice 1 — persistence foundation

Implement the closed candidate schema, dedicated storage owner, bounded
load/save/delete operations, validation, atomic replacement, coordination,
corruption handling, future-version handling, and deterministic persistence
tests. This is the Task 331 slice. It has no UI, no startup admission, no
activation, and no provider/runtime integration.

### Slice 2 — inert startup catalog

Load remembered candidates during Desktop startup into a presentation-safe
in-memory catalog. Prove with deterministic restart tests that valid records
restore no members, active repository, repository composition, provider,
runtime, Codex, ToolRegistry, Effective Authority, Commit, Stage/Unstage, or
HostExplicit state. Preserve the passive-probing rule: no candidate-path
filesystem or Git work occurs merely because a hint is present.

### Slice 3 — catalog mutation and fresh re-admission bridge

Add explicit remember/update/delete/reorder actions and the explicit human
fresh-admission bridge from a stored location hint or fresh path selection.
Rerun all current ADR 0027 admission checks. Allocate a new process-local
`RepositoryMemberId` and fresh currentness only after successful admission;
keep the member inert until explicit activation. Do not automatically activate
or reconnect anything.

### Slice 4 — Desktop UX and certification

Add frontend presentation and explicit human actions through privacy-safe
closed IPC DTOs. Keep raw hints redacted by default, keep frontend state
non-authoritative, verify restart behavior and all privacy surfaces, and run
the separately authorized Windows live certification. This slice does not add
member removal of current process-local admitted members.

## 8. Deterministic acceptance tests

Task 331 tests must cover at least:

- valid minimal and opt-in-hint v1 records;
- unknown/duplicate/missing/null/wrong-type/trailing/malformed input;
- invalid IDs, labels, hints, references, counts, nesting, and byte bounds;
- unique durable ID generation independent of path and process-local member
  identity;
- missing, corrupt, truncated, oversized, unsupported-version, and storage
  ownership outcomes;
- complete staged-byte reparse before replacement;
- old-file preservation for create/write/flush/close/coordination/replacement
  failures;
- bounded cleanup of only store-owned temporary files;
- best-effort corruption quarantine without silent empty replacement; and
- proof that load/save/delete have no repository, Git, provider, runtime,
  ToolRegistry, authority, or frontend side effect.

Later slices must add restart no-authority tests, passive-probing instrumentation,
fresh admission identity/currentness tests, catalog deletion isolation,
privacy sentinel tests across disk/IPC/UI/log/error/activity/Effective
Authority/model/Codex/MCP/Process Plugin/diagnostic surfaces, and Windows live
certification. The existing 11-name HostExplicit set, Codex `0.149.0` baseline,
ADR 0027 behavior, conversation persistence, and all v0.26 authority contracts
remain regression requirements.

## 9. Explicit exclusions

This contract does not authorize automatic admission, automatic activation,
startup probing, authority restoration, persisted identity proof, frontend
changes in Task 331, provider integration, HostExplicit changes, Commit or
Stage/Unstage integration, Codex changes, new permissions, new Tools,
dependency/Cargo changes, conversation schema changes, path search, cloud or
network synchronization, custom encryption, OS sandboxing, network isolation,
rollback, recovery journaling, or effect replay.

## Task 331 — Remembered Workspace Persistence Foundation

Task 331 is a real production implementation task with the intentionally
narrow scope of candidate data structures; the persistent closed v1 schema;
the dedicated `RememberedWorkspaceStore`; bounded load/save/delete;
corruption, unsupported-version, invalid-input, and bounds handling; atomic
same-directory replacement and bounded synchronization; and deterministic
tests for those behaviors.

Task 331 must not yet add automatic admission, automatic activation, frontend
UX, provider integration, HostExplicit changes, Commit/Stage integration, or
Codex changes.
