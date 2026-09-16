# ADR 0028 — Remembered Workspace Persistence Is Descriptive, Not Repository Authority

Status: Accepted

Date: 2026-09-16

## Context

ADR 0027 accepts host-owned descriptive workspace membership, fresh
process-local repository identity, exactly one active repository, and fresh
active-only authority composition. It also requires restart to restore no
executable repository state. Task 329 researched the narrower question of how
to remember workspace candidates across process restarts without weakening
that boundary.

The result is a durable catalog of human-curated descriptive candidates. The
catalog is private host data below repository authority. It is not a workspace
root, a repository identity, a permission, a currentness record, a recovery
mechanism, or a serialized authority handle.

## Decision

RAH v0.27 accepts a dedicated, host-owned remembered-candidate catalog with
this only executable transition:

```text
remembered candidate
  -> explicit human fresh admission
  -> existing full repository identity verification
  -> new process-local RepositoryMemberId
  -> explicit activation
  -> fresh active-only authority composition
```

Remembered workspace persistence is durable descriptive state, not repository
authority. A remembered record may survive process restart. Executable
repository authority may not.

After restart, the required state is:

```text
remembered candidates may exist
but
admitted members = 0
active repository = none
DesktopRepository = none
repository ToolRegistry = none
Commit authority = none
Stage/Unstage authority = none
HostExplicit preparation/tickets = none
runtime/provider repository binding = none
```

This ADR is additive. ADR 0027 remains unchanged and continues to own
process-local repository membership, repository identity/currentness, active
composition, switching, the one-active-repository rule, no-union registry
behavior, and repository-bound authority. This ADR owns only durable
remembered candidates, their storage/privacy contract, restart semantics, the
fresh re-admission boundary, and descriptive catalog mutation.

## Durable candidate identity

Each persisted candidate has a `RememberedCandidateId`. It is a bounded,
host-generated random opaque identifier. It is persistent and descriptive
only. It is not derived from a path, repository name, filesystem identity,
Git hash, credential, provider value, or authority handle.

`RememberedCandidateId` and `RepositoryMemberId` are different types with
different lifetimes and meanings. A durable candidate ID is never accepted by
repository admission, repository-bound Tool APIs, currentness checks,
activation, HostExplicit operations, Commit, Stage/Unstage, provider, or
runtime APIs. It is never reused as a process-local member ID, a continuity
proof, or a currentness key.

## Closed durable schema

The v0.27 format is a dedicated closed JSON record, schema version `1`. It is
not an extension of `desktop-preferences.json` or
`conversation-transcript.sqlite3`.

The v1 shape is:

```text
Root      = { version: u64, workspace: Workspace }
Workspace = { id: OpaqueId, label: Label, members: Member[],
              last_active_member_id?: OpaqueId }
Member    = { id: OpaqueId, label: Label, location_hint?: PathHint }
```

In the durable schema, `Member.id` is the `RememberedCandidateId`; the
`members` name is descriptive and must not be confused with an admitted
process-local repository member.

The field classification is:

| Field | Classification | Contract |
| --- | --- | --- |
| `version` | Required | Exactly integer `1`. |
| `workspace.id` | Required | Bounded host-generated opaque descriptive ID. |
| `workspace.label` | Required | Bounded, control-free descriptive label. |
| `workspace.members` | Required | Ordered candidate records, bounded to 64 entries. Array order is presentation order only. |
| `members[].id` | Required | Unique bounded `RememberedCandidateId`. |
| `members[].label` | Required | Bounded, control-free user label. |
| `members[].location_hint` | Optional | Explicitly opted-in bounded absolute/native location hint. Absent by default. |
| `workspace.last_active_member_id` | Optional | Descriptive presentation/focus hint referring to a member in the same record. It never activates or validates. |
| Any other field | Forbidden | Unknown, duplicate, null, authority-shaped, or future extension fields fail closed. |

The root and every nested object accept exactly the listed keys. Missing
required fields, duplicate keys, unknown keys, nulls, wrong types, malformed
UTF-8, trailing data, invalid control characters, invalid IDs, invalid
references, and invalid bounds fail closed. A future schema version is not
downgraded, partially read, or rewritten by a v1 implementation.

The initial independent bounds are:

| Value | Bound |
| --- | ---: |
| Serialized final file | 256 KiB |
| Workspace members | 64 |
| Workspace/member label | 256 UTF-8 bytes each |
| Opaque ID | 64 ASCII bytes |
| Optional location hint | 4096 UTF-8 bytes |
| Object/array nesting | Fixed schema depth only |

Bounds are resource and storage limits, not permissions or authority.

The schema must never contain, directly or through a future authority-shaped
field:

- `RepositoryMemberId`, `workspace_epoch`, or any live membership, admission,
  repository, active-selection, composition, or currentness generation;
- filesystem identity proof, canonical repository root proof, `.git` form or
  identity proof, or Git executable identity proof;
- `DesktopRepository`, `ToolRegistry`, Effective Authority, permission or D2
  decisions, Commit capability/authorization, Stage/Unstage action IDs, or
  index-effect reservations;
- HostExplicit tickets or preparations;
- provider/runtime handles, provider credentials, Codex runtime, connection
  currentness, or executable conversation repository context; or
- Tool definitions, Tool input/output, transcript content, effect state,
  replay state, rollback state, or recovery authority.

## Location hints and storage privacy

Repository location is private host data. `location_hint` is absent by
default. It is persisted only when the human explicitly opts in to remember
that location for a descriptive candidate. Admitting or activating a
repository does not silently persist its path.

When present, a hint is only a bounded platform-representable absolute path
string with no NUL or control characters. Persistence retains it as a user
locator and performs no canonicalization, filesystem probing, `.git`
inspection, identity derivation, or availability check. An unrepresentable or
over-limit hint is rejected or omitted as descriptive input; it never grants
authority. No path hash or other location fingerprint is stored as a
continuity substitute.

The privacy contract is:

| Surface | Required treatment |
| --- | --- |
| Disk | An explicitly opted-in hint may exist only in the dedicated catalog in the normal per-user app-data directory. Normal account storage is the at-rest boundary; no encryption claim is made. No identity or authority proof is stored. |
| Frontend IPC | Catalog IPC uses a closed presentation DTO. It may carry the opaque descriptive ID, labels, ordering, and redacted location presentation. A full hint may cross only a dedicated explicit human-facing show-location action; it is never accepted as authority metadata. |
| UI | Show a redacted location by default. A full hint requires an explicit host-only privacy action. UI presentation and controls do not admit, activate, or authorize a repository. |
| Logs | Do not log the raw hint, storage path, identity evidence, tickets, preparations, credentials, Tool data, or source-bearing content. |
| Errors | Return bounded category/status errors without the raw hint, native storage path, identity evidence, or authority values. |
| Activity events | Status-only descriptive activity may identify a bounded catalog outcome, but must not contain the raw hint, candidate authority-shaped values, Tool input/output, tickets, preparations, or source content. |
| Effective Authority | Never include remembered candidates, location hints, candidate IDs as authority, storage paths, or any persisted currentness/identity value. |
| Model | A remembered candidate and its location are never model-visible repository selection or authority input. |
| Codex | The Codex runtime receives no catalog record, location hint, candidate ID as a repository selector, or restart state. |
| MCP | MCP configuration and metadata never receive remembered candidate data or location hints. |
| Process Plugin | Process Plugin configuration, metadata, and Tool traffic never receive remembered candidate data or location hints. |
| Diagnostics/tests | Sentinel hints, identities, credentials, tickets, and authority handles are host-test inputs only. Diagnostics remain bounded and sanitized; tests verify actual serialized and emitted values, not only field names. |

Persisting a repository location does not make it model- or provider-visible.
The dedicated human workspace view is the only permitted presentation context
for a full opted-in hint, and that presentation remains non-authoritative.

## Storage owner and write contract

`RememberedWorkspaceStore` is the single storage and reconciliation owner for
the catalog. It owns the dedicated file `remembered-workspace.json` in the
existing per-user application-owned data directory. It owns closed-schema
validation, bounded load/save/delete, in-process catalog synchronization,
atomic replacement, corruption handling, and bounded storage errors. It does
not own repository admission, activation, ToolRegistry composition, provider
lifecycle, or runtime lifecycle.

The catalog is isolated from `desktop-preferences.json`,
`conversation-transcript.sqlite3`, repository/workspace roots, neutral
execution directories, provider profiles, and shared/network locations. The
store must not deliberately relax inherited app-data permissions and must
fail closed rather than follow a file or parent that violates the supported
local app-data contract, including unsupported link/reparse/device forms.

The store serializes descriptive catalog mutations inside the process. It
also performs a bounded OS/file coordination attempt for the catalog file. If
exclusive coordination cannot be obtained, the operation returns
`save_failed` and does not replace the final file. There is no catalog merge
and no unsafe last-writer-wins assumption.

Every successful replacement follows this contract:

1. Validate the complete in-memory closed v1 record and serialized bound.
2. Create a uniquely named, store-owned temporary file in the same directory.
3. Write, flush/synchronize, and close the complete bytes successfully.
4. Validate the staged bytes with the same closed parser.
5. Perform one supported native same-directory replacement of the final file.
6. Report success only after replacement succeeds; on failure, clean up only
   the owned temporary file.

The old final record remains authoritative until replacement succeeds. Create,
write, flush, close, coordination, replacement, and cleanup failures are
bounded `save_failed` results. A failed save does not publish partial bytes,
retry, replay, auto-admit, refresh authority, or roll back repository
authority. Temporary cleanup may occur only for stale files matching the
store's exact owned temporary naming rule.

Windows locking and replacement behavior must be treated as explicit bounded
failure cases. The store must not hold a catalog lock while running Git,
probing a candidate path, connecting a provider, executing a model turn, or
performing repository mutation.

## Restart and startup semantics

Startup may open and parse the one bounded dedicated catalog. The only
permitted path is:

```text
open dedicated catalog
  -> bounded closed parse
  -> inert remembered presentation
  -> zero repository members
  -> zero active repository
  -> zero repository-bound executable composition
```

Startup must not, because a hint exists:

- stat/open/canonicalize a candidate location or resolve a reparse target;
- inspect `.git`, perform nested-repository discovery, invoke Git, or probe
  removable/offline/network media;
- allocate a `RepositoryMemberId`, admit a repository, construct a
  `DesktopRepository`, or create a repository generation;
- select or activate a repository, create a ToolRegistry, publish Effective
  Authority, or restore Commit, Stage/Unstage, HostExplicit, or index state;
- reconnect a provider, restore a runtime/Codex handle, advertise Tools, or
  select a transcript namespace from the hint; or
- search for moved locations, retry unavailable candidates, or recover an
  uncertain effect.

Automatic re-admission is forbidden for v0.27. A stored path/location hint is
not consent, authority, identity proof, currentness, or an activation request.
Automatic activation is also forbidden. A remembered last-active candidate is
only a UI focus hint and must never build a registry, activate a repository,
connect Codex, or restore Commit/Stage/HostExplicit state.

## Fresh re-admission and activation

An explicit human selection of a remembered candidate is a new admission
request, not restoration. The host obtains the location from the opted-in hint
or from a fresh picker/typed path and reruns the existing ADR 0027 admission
pipeline from scratch:

- canonicalization and supported path-form checks;
- filesystem identity verification;
- `.git` form and identity validation;
- Git executable identity validation;
- ordinary-worktree requirements;
- duplicate and case/alias-equivalence checks;
- nested repository boundary checks; and
- applicable repository policy checks.

Only successful validation creates a new process-local `RepositoryMemberId`, a
fresh admission generation, fresh membership/currentness generations, and an
ordinary inert process-local member. Explicit activation remains a separate
human action. It withdraws old executable composition and builds one fresh
active-only composition through ADR 0027. No durable field can skip either
step.

Unavailable, moved, replaced, aliased, nested, unsupported, or inaccessible
candidates remain inert with bounded fail-closed outcomes. The host does not
search for a moved path or claim continuity with a repository at a changed
location. A different repository explicitly admitted at the same path may be
descriptively rebound only after successful fresh admission, without claiming
continuity with the old candidate.

## Descriptive catalog operations

V0.27 supports these catalog-only operations:

- `remember/add`: an explicit human action creates a candidate with a fresh
  durable ID, label, and optionally an explicitly opted-in location hint;
- `update`: an explicit human action changes descriptive label or location
  hint. Changing a hint does not probe, admit, activate, or rebind authority;
- `reorder`: an explicit human action changes array presentation order; and
- `delete`: an explicit human action removes the durable descriptive record.

The optional `last_active_member_id` is a descriptive focus hint and may be
updated by an explicit catalog operation. It is never an activation command.

Deleting a remembered candidate does not delete filesystem content, mutate
Git, remove an admitted process-local member, deactivate the active
repository, revoke current authority, mutate ToolRegistry, or mutate Commit,
Stage/Unstage, HostExplicit, provider, runtime, or conversation state. Durable
candidate deletion and process-local member removal are separate future
features. No v0.27 operation removes a current process-local admitted member.

If executable admission succeeds but a subsequent descriptive catalog save
fails, the completed current-process admission transaction remains governed by
its existing authority boundary. The persistence operation reports
`save_failed`; it does not invent a rollback, replay the save, undo admission,
or alter repository authority. A descriptive catalog mutation that cannot be
persisted is reported as a bounded storage failure and must not be represented
as durable success.

## Corruption, bounds, and migration

The store distinguishes these outcomes:

| Condition | Result |
| --- | --- |
| Valid v1 file | Load bounded inert candidates. No rewrite until an explicit later mutation. |
| Missing file | Empty remembered catalog. Do not create a file at startup. |
| Oversized, malformed, truncated, wrong-type, duplicate, unknown-field, invalid-bound, or invalid-location file | No candidates and bounded `catalog_unavailable`. Never trust a partial record. |
| Unsupported future version | No candidates and bounded `unsupported_version`. Preserve the source; do not downgrade or rewrite it. |
| Storage ownership/coordination failure | No candidates for this process and bounded storage failure. |

Malformed or structurally invalid data may be renamed to a same-directory
`.corrupt` quarantine name as a best-effort preservation step after the source
is closed. If quarantine fails, leave the original intact and expose only a
bounded warning. Never replace corruption with an empty record. Quarantine
must match the store's exact naming rule and must not remove unrelated files.

Migration is the simple closed-schema v1 strategy: there is no generic
migration framework. A later schema requires a separately accepted,
loss-aware migration that parses the source under its own closed rules,
validates a new record, preserves a recoverable source, and atomically
replaces only after successful validation. V1 must not drop unknown fields,
guess meanings, import other schemas, or restore authority.

Corruption, quarantine, unsupported versions, invalid paths, and persistence
failures may lose or isolate descriptive data, but they can never manufacture
membership, identity, currentness, activation, provider/runtime state, or
repository authority.

## Relationship and nonclaims

ADR 0027 remains valid and unchanged. This ADR does not alter its switching,
identity, nested-boundary, currentness, active-only composition, or
repository-bound authority contracts. It does not alter ADR 0003, ADR 0011,
ADR 0016, ADR 0018, ADR 0021, or ADRs 0022–0027.

The exact HostExplicit set remains the same 11 names:

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
repo.rename-file
```

This ADR adds no Tool, permission, HostExplicit eligibility, provider,
network, Codex, frontend authority, Cargo, dependency, migration, cloud-sync,
custom-encryption, OS-sandbox, network-isolation, rollback, recovery, replay,
or cross-repository capability. It does not implement the store, startup
integration, fresh admission bridge, frontend, provider integration, or live
certification.

## Acceptance

This ADR accepts only the durable descriptive catalog and restart contract
defined above. Task 331 may implement the persistence foundation directly
against its companion implementation contract. Any proposal to persist
identity proof, auto-probe, auto-admit, auto-activate, restore authority,
select a repository from model/provider input, or replay an uncertain effect
requires a separate authority/security decision.

References: Task 329 research in
`docs/plans/2026-09-15-task-329-remembered-workspace-persistence-authority-privacy-research.md`;
ADR 0027 in
`docs/adr/0027-workspace-repository-identity-authority-composition.md`.
