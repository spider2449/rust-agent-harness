# Task 329 - Remembered Workspace Persistence Authority and Privacy Research

Date: 2026-09-15
Task type: research only
Status: READY FOR TASK 330 ADDITIVE ADR RESEARCH/DECISION

Starting checkpoint: `f4d4bb68a8c404777ec0aeebe6ab1fc5dcd0f274`
(`chore: normalize generated desktop permission line endings`; exact-head CI
`35042244235` passed). This checkpoint is the direct descendant of the Task
328 source commit `b4493170cc6a755634a6eee49cfdfe9564ea700e`.

This document researches the authority, privacy, persistence, and restart
contract for Task 328 Recommendation B: a durable remembered multi-repository
workspace. It does not implement Rust, frontend behavior, Cargo metadata,
storage, migration, startup reconciliation, repository removal, an ADR, or
release changes. It does not modify the `.gitattributes` line-ending rule or
the generated permission TOMLs.

## 1. Research conclusion

The selected design is a host-owned remembered-candidate catalog below
executable authority:

```text
remembered descriptive candidate
  -> explicit fresh host admission
  -> fresh process-local repository member identity
  -> explicit activation
  -> fresh active-only repository composition
```

The durable catalog is not a workspace root, repository identity, permission,
ToolRegistry, profile, runtime, currentness record, or recovery mechanism.
Restart creates zero repository membership authority. A remembered record can
make a candidate visible to the human, but it cannot admit, validate, activate,
connect, compose, spawn, advertise, or restore any repository-bound state.

The recommended location policy is opt-in per candidate. The default record
contains no absolute/native path. A user may explicitly request remembering a
location hint for a candidate; the hint remains sensitive descriptive data,
not identity proof. It is never required for the catalog to remain valid and
is never used without a fresh explicit host action.

This is sufficiently defined for Task 330 to make an additive persistence and
restart authority decision. It is not authorization to implement Task 331.

## 2. Authoritative boundaries and current facts

The following existing decisions remain unchanged:

- ADR 0027 makes workspace membership descriptive, permits inert descriptive
  persistence, requires fresh process-local repository identity, and permits
  at most one active repository composition.
- Repository admission is host-owned. Model output, provider metadata,
  frontend fields, persisted values, and Tool input cannot admit or select a
  repository.
- Fresh admission validates the canonical root, supported `.git` form and
  identity, Git executable identity, ordinary-worktree state, nested
  boundaries, alias/duplicate relationships, and applicable repository
  policy.
- Active switching withdraws old executable state and builds a fresh
  repository-bound composition. Inactive candidates do not contribute a
  registry or Effective Authority.
- HostExplicit tickets, preparations, Commit authorization, Stage/Unstage
  actions, provider state, runtime state, ToolRegistry state, and currentness
  generations are process-local and nonpersistent.
- No new Tool, PermissionLevel, HostExplicit eligibility, provider authority,
  network authority, or Codex baseline is introduced.

The current Desktop implementation has separate owners for private inactive
preferences, repository membership, and conversation persistence. The
remembered workspace catalog must have its own storage and reconciliation
owner. Existing preference and transcript schemas must not be reused merely
because they already write under the application-data directory:

- `desktop-preferences.json` stores model, commit-identity, and inert Trusted
  Profile preferences with a different lifecycle and privacy contract.
- `conversation-transcript.sqlite3` stores bounded conversation history under
  repository or neutral namespaces and is source-bearing user content.
- `WorkspaceMembershipState` is process-local and contains fresh admission
  identity/currentness that must never be serialized into the remembered
  catalog.

## 3. Authority invariant

The implementation and every test must preserve these distinct states:

| State | Durable | Can read repository state | Can create membership authority | Can activate | Can supply Tool authority |
| --- | --- | --- | --- | --- | --- |
| Remembered candidate | descriptive fields only | no | no | no | no |
| Fresh-admission request | no; host action only | bounded host validation | only after successful validation | no | no |
| Process-local admitted member | no | only through its fresh host binding | already admitted | no, unless explicitly selected | inactive: no |
| Active repository | no | yes, under current binding | already admitted | explicitly active | yes, through existing composition |

Persisted data must never be interpreted as a capability handle, proof of
continuity, proof that a path still exists, or consent to restore authority.
No startup branch may jump from the first row to any later row.

## 4. Threat model

### 4.1 Protected assets

The design protects:

- absolute/native repository location hints and user labels;
- repository root, `.git`, filesystem, and Git executable identity evidence;
- repository membership, active selection, authority composition, and
  currentness generations;
- credentials, provider configuration, runtime state, Tool definitions and
  inputs, HostExplicit tickets/preparations, Commit authorization, and
  uncertain-effect state; and
- source-bearing conversation or repository content that must remain in its
  existing bounded owner.

### 4.2 In-scope adversaries and failures

The catalog must fail closed against:

- a malformed, truncated, oversized, duplicated, unknown-field, wrong-type,
  or unsupported-version file;
- a crash or power loss during replacement, a partial temporary file, or a
  failed synchronization/replacement operation;
- a stale candidate whose path was moved, deleted, replaced, aliased, made
  inaccessible, placed on removable/offline media, or changed in `.git` form;
- a path that resolves into a nested repository, an alias-equivalent
  duplicate, or an unsupported reparse/link/device form;
- a persisted value attempting to smuggle identity, generation, authority,
  credential, provider, Tool, or executable fields through a future schema;
- a model, provider, or frontend attempting to turn a remembered value into a
  repository selector or authority source; and
- multiple writers or stale application instances replacing the catalog
  without an explicit synchronization result.

The privacy boundary also assumes that a location hint may be disclosed by
the normal user-account application-data boundary, OS backup, or a user with
access to that account. The design does not claim protection from same-account
malware, an administrator, a compromised host, or a deliberately shared
application-data directory.

### 4.3 Trusted boundary and nonclaims

The trusted composition boundary remains host code plus the operating-system
user/application-data boundary. The catalog is not cryptographically
authenticated authority and is not a secret store. No custom encryption,
key-management, network sync, cloud backup, or cross-user sharing is part of
this task. A validly tampered descriptive record still cannot restore
authority because fresh host admission is mandatory.

The design does not claim race-free TOCTOU behavior, OS sandboxing, network
isolation, rollback of repository or provider effects, automatic recovery,
effect replay, or durable continuity of any external operation.

## 5. Closed descriptive schema

The recommended initial storage format is a dedicated closed JSON record. The
record is version `1`; it is not an extension of the desktop preferences or
conversation schemas.

Illustrative valid content, with an explicitly opted-in location hint, is:

```json
{
  "version": 1,
  "workspace": {
    "id": "opaque-workspace-id",
    "label": "Personal projects",
    "members": [
      {
        "id": "opaque-member-id",
        "label": "Harness",
        "location_hint": "D:\\work\\harness"
      }
    ],
    "last_active_member_id": "opaque-member-id"
  }
}
```

The exact closed contract is:

```text
Root              = { version: u64, workspace: Workspace }
Workspace         = { id: OpaqueId, label: Label, members: Member[]
                      last_active_member_id?: OpaqueId }
Member            = { id: OpaqueId, label: Label, location_hint?: PathHint }
```

The following rules are part of the proposed ADR contract:

- The root and every nested object accept exactly the listed keys. Unknown or
  duplicate keys, missing required fields, `null`, trailing data, malformed
  UTF-8, and wrong JSON types fail the whole record.
- `version` must be exactly `1`. A future version is not downgraded, partially
  read, or rewritten by an older binary.
- `OpaqueId` is a bounded host-generated random identifier. It is not derived
  from a path, repository name, filesystem identity, Git hash, credential, or
  authority handle. It has no admission or currentness meaning after restart.
- `label` is bounded, control-free, user-facing descriptive text. A label is
  not a repository name proof and is not authority metadata.
- The member array is bounded and has unique member IDs. Array order is the
  descriptive presentation order; it does not represent activation order or
  authority priority.
- `last_active_member_id`, when present, is only a presentation/focus hint.
  It must refer to a record member, but it never causes startup activation or
  validation and may be discarded if the candidate is unavailable.
- No `admitted`, `active`, `available`, `validated`, `current`, `generation`,
  `repository_root_identity`, `.git` identity, Git executable identity,
  `ToolRegistry`, provider, runtime, profile, credential, ticket,
  preparation, Commit, Stage/Unstage, model, transcript, or effect field is
  allowed.

### 5.1 Bounds

The proposed initial bounds are deliberately small and independently
testable:

| Value | Bound |
| --- | ---: |
| Serialized final file | 256 KiB |
| Workspace members | 64 |
| Workspace/member label | 256 UTF-8 bytes each |
| Opaque ID | 64 ASCII bytes |
| Optional location hint | 4096 UTF-8 bytes |
| Array and object nesting | fixed schema depth only |

The serialized-file bound is checked before parsing and again before writing.
Every individual string and count is bounded before allocation or publication.
The bounds are storage/resource limits, not authority or permission limits.

### 5.2 Location-hint policy

The default is no `location_hint`. Presence means the user explicitly opted in
to remembering a location for that candidate. A host UI may offer this as a
separate, clearly labelled privacy choice; merely admitting or activating a
repository must not silently persist its path.

When present, the hint is:

- a bounded platform-representable absolute path string with no NUL or control
  characters;
- retained as a user locator without canonicalization, filesystem probing,
  `.git` inspection, or identity derivation during persistence or startup;
- rejected or omitted, without invalidating the rest of the catalog, if it
  cannot be represented under the selected storage encoding or bound; and
- never treated as a root identity, repository hash, continuity proof, or
  permission grant.

The dedicated human workspace view may show a redacted path by default and a
full hint only as an explicit host-only privacy action. The raw hint must not
appear in generic activity, warnings, crash text, model prompts, transcript
rows, provider metadata, Tool definitions, Tool input/output, or Effective
Authority. A user label is similarly excluded from model/provider authority
surfaces unless a separately reviewed presentation contract says otherwise.

No path hash is stored as a substitute: it is still a durable location
fingerprint, cannot safely restore identity, and would create a misleading
continuity signal.

## 6. Dedicated storage and at-rest protection

The catalog should use a dedicated file and owner, for example
`remembered-workspace.json`, in the existing app-owned data directory. The
exact filename is a Task 330 contract detail, but the following separation is
mandatory:

- Do not add workspace candidates to `desktop-preferences.json` as an
  unrelated optional object.
- Do not store them in `conversation-transcript.sqlite3` or any transcript
  namespace.
- Do not make the catalog a general settings database or a model/tool SQL
  capability.
- Do not place it in the repository, workspace root, neutral execution
  workspace, provider profile directory, or a shared/network location.

The storage owner must use the normal per-user application-data boundary and
must not deliberately relax inherited file or directory permissions. A
future implementation must reject or fail closed on a final file or parent
that is a link/reparse/device form outside the supported app-data contract,
and must not follow an attacker-selected storage redirect. It must not claim
that normal user-account storage encrypts the path.

At startup, reading and parsing this one bounded file is allowed. Startup must
not read any candidate path, call repository discovery, inspect `.git`, run
Git, resolve a reparse target, probe removable media, or validate provider or
runtime state merely because a hint is present.

## 7. Atomicity, synchronization, and failure semantics

The storage owner must serialize writes inside the process and define its
behavior when another process owns the file. The recommended behavior is a
bounded OS/file coordination attempt: if exclusive coordination cannot be
obtained, return `save_failed` without replacing the existing final file.
There is no merge of two catalogs and no assumption that last-writer-wins is
safe for user curation.

The recommended write sequence is:

1. Validate the complete in-memory closed record and serialized byte bound.
2. Create a uniquely named temporary file in the same app-data directory.
3. Write the complete bytes, flush/synchronize the temporary file, and close
   it successfully.
4. Re-read or otherwise validate the staged bytes through the same closed
   parser before replacement.
5. Perform one native same-directory replacement of the final file, using the
   supported platform fallback rules already established for Desktop local
   persistence.
6. Report success only after replacement succeeds. Clean up only the owned
   temporary file on a failed attempt.

The old final record remains authoritative until the replacement succeeds.
Create, write, flush, close, coordination, replacement, and cleanup failures
are bounded `save_failed` outcomes; they must not publish a partially written
catalog or alter in-memory authority. A failed save never triggers a retry,
replay, automatic admission, or authority refresh.

Temporary names must be recognizably owned by this storage owner. Startup may
remove only stale temporary files matching that owner and exact naming rule;
it must not delete arbitrary files from the app-data directory. A replacement
failure must not cause generated permission files or unrelated repository
files to be examined, normalized, or changed.

## 8. Corruption, versioning, and migration

The storage owner must distinguish corruption from a future valid version:

| Condition | Startup result | Destructive rewrite allowed? |
| --- | --- | --- |
| Valid version 1 | Load bounded inert candidates | No rewrite unless an explicit later mutation occurs |
| Missing file | Empty remembered catalog | Create only on an explicit successful save |
| Oversized, malformed, truncated, wrong-type, duplicate, unknown-field, or invalid-bound file | No candidates; bounded `catalog_unavailable` result | No; quarantine only under the rule below |
| Unsupported future version | No candidates; bounded `unsupported_version` result | No |
| Valid file but failed storage ownership/coordination check | No candidates for this process | No |

Malformed or structurally invalid input may be renamed to a same-directory
`.corrupt` quarantine name as a best-effort diagnostic preservation step only
after the original is closed. If quarantine fails, leave the original intact
and expose only a bounded warning. Never replace a corrupt record with an
empty record merely to make startup look successful.

An unsupported future version is not automatically quarantined or rewritten:
an older binary must preserve data it cannot interpret. A future migration
must be an explicit, bounded, loss-aware migration owned by a later task. It
must parse the source with the source version's closed rules, write a new
validated temporary record, atomically replace only after validation, and
retain a recoverable source copy according to a separately accepted policy.
There is no best-effort field dropping, unknown-field preservation, or
cross-schema import in Task 329.

Corruption and migration outcomes are descriptive only. Neither may create a
member, select a repository, restore a provider, restore a conversation, or
continue an uncertain effect.

## 9. Restart and fresh-admission contract

### 9.1 Startup state machine

The only permitted startup route is:

```text
open dedicated catalog
  -> bounded closed parse
  -> inert remembered presentation
  -> zero repository members
  -> zero active repository
  -> zero repository-bound executable composition
```

Even a valid candidate with a valid-looking location hint must not cause:

- path existence or filesystem identity checks;
- `.git` or nested-boundary discovery;
- Git executable discovery or status probing;
- repository admission or member-ID allocation;
- active selection, `DesktopRepository` construction, or repository generation;
- ToolRegistry or Effective Authority publication;
- provider connection, runtime restoration, or Tool advertisement;
- transcript namespace selection based on the hint; or
- automatic retry/recovery for a missing or unavailable location.

`last_active_member_id` may affect only which inert row receives presentation
focus. It is not an instruction to activate that row.

### 9.2 Explicit fresh admission

A human may explicitly choose a remembered row. That action is a new admission
request, not a restore operation. The host obtains the candidate path from the
opt-in hint or from a fresh picker/typed path and then performs all current
ADR 0027 admission checks. The persisted member ID is not reused as the
process-local member ID; a fresh identity, admission generation, and current
repository binding are created only after validation succeeds.

The host must publish the candidate as a new process-local member before any
activation. Explicit activation then withdraws any old active composition and
creates exactly one fresh active-only composition through existing boundaries.
No persisted field can skip either step.

### 9.3 Reconciliation outcomes

Because filesystem and Git identity proof is intentionally not persisted, a
new process cannot prove that a path still refers to the old physical
repository. This is a deliberate privacy and authority tradeoff, not a gap to
be repaired by storing a hash or identity tuple. The UI and host outcomes are:

| Situation when the human attempts fresh admission | Required outcome |
| --- | --- |
| No location hint | `fresh_admission_required`; use a new host-selected path |
| Hint is missing or inaccessible | `unavailable`; no search or retry |
| Hint points to a current supported root | Run full fresh admission; success creates a new member, failure remains inert |
| Root or `.git` was replaced or changed | No continuity claim; only the current fresh-admission checks decide, otherwise `fresh_admission_required` |
| Git executable changed or is unsupported | Fresh admission fails closed; no old binding is revived |
| Case/alias-equivalent duplicate | `already_member` or `duplicate_candidate`; no focus/activation side effect |
| Nested repository relationship | `nested_member_conflict` or applicable repository-bound rejection |
| Removable/offline media or access failure | `unavailable`; no automatic recovery |
| Corrupt or unsupported catalog | `catalog_unavailable` or `unsupported_version`; use explicit new admission |

If a user explicitly admits a different repository at the same path, the host
may update the descriptive candidate only after successful admission and
explicitly rebind the user-facing label. It must not claim continuity with
the prior repository. A moved repository is handled by a new host-selected
path and fresh admission; the old hint is not searched for or followed.

When multiple persisted rows contain aliases or duplicate-looking hints, the
catalog may preserve them as inert descriptive rows because startup performs
no path identity checks. Fresh admission must apply current duplicate and
nested-boundary rules against process-local members; persistence itself must
never manufacture a union or cross-repository authority.

## 10. Privacy and authority test requirements

Task 330 must turn these research requirements into an accepted contract;
implementation tasks must then provide deterministic tests at the narrowest
applicable layer.

### 10.1 Schema and resource tests

- Accept one valid minimal record and one record with an explicitly opted-in
  location hint.
- Reject unknown/duplicate fields, missing fields, nulls, wrong types,
  trailing bytes, malformed UTF-8, control characters, invalid IDs, invalid
  bounds, duplicate IDs, invalid last-active references, and oversized input.
- Prove that parsing bounds allocations and never accepts unknown future
  authority-shaped fields.
- Prove missing, malformed, corrupt, and unsupported-version outcomes do not
  create process-local membership or active state.

### 10.2 Atomic storage tests

- Inject create, write, flush, close, coordination, replacement, and cleanup
  failures.
- Prove the old final record remains byte-for-byte intact unless one complete
  replacement succeeds.
- Prove owned temporary cleanup is bounded and no unrelated file is removed.
- Prove concurrent/second-process behavior is an explicit bounded failure,
  not an implicit merge or authority race.
- Prove corrupt quarantine preserves recoverable bytes and never silently
  replaces them with an empty catalog.

### 10.3 Restart and authority tests

- Restart with valid remembered rows and assert zero repository members,
  zero active repository, zero repository generation, zero repository-bound
  registry/effective authority, and no provider/runtime restoration.
- Instrument startup to prove it performs no candidate-path stat/open,
  `.git` inspection, Git invocation, nested scan, or automatic admission.
- Prove a remembered member ID and last-active hint cannot be used as a
  process-local member ID, repository identity, currentness generation, or
  activation command.
- Prove explicit admission freshly validates root, `.git`, Git executable,
  alias/duplicate, nested, and ordinary-worktree conditions before creating
  a member, and explicit activation composes only the selected repository.
- Prove A-to-B and A-to-B-to-A switching retains existing invalidation and
  currentness behavior; no persisted row revives old A state.

### 10.4 Privacy boundary tests

Use unique sentinel workspace labels, member labels, location hints, fake
filesystem identities, fake Git identities, credentials, ticket values, and
authority handles. Verify that:

- only the explicitly opted-in location hint is present in the dedicated
  catalog, and no identity/proof/authority value is serialized;
- bounded storage errors and generic activity contain no raw hint, identity,
  ticket, preparation, Tool input/output, or source-bearing review;
- model prompts, transcript rows, provider metadata, Tool definitions,
  Effective Authority, and live evidence contain no remembered path or
  authority-shaped catalog value;
- the dedicated human view exposes only the intended redacted/full path
  behavior and cannot submit authority metadata as a command; and
- a malicious catalog cannot inject control characters, path traversal,
  provider configuration, model text, or frontend authority fields.

### 10.5 Regression tests

The remembered catalog must not change existing v0.26 behavior for:

- active-only repository composition and one-active-repository switching;
- nested repository and alias/duplicate rejection;
- repository-bound reads, mutations, index effects, reviewed Commit, and
  HostExplicit tickets;
- Trusted Profile, provider, runtime, and Codex lifecycle;
- conversation persistence namespace and explicit Resume semantics; or
- the exact 11 HostExplicit Tool set and certified Codex baseline.

## 11. Task 330 acceptance gate

Task 330 may accept an additive persistence/restart ADR only if it records all
of the following without weakening ADR 0027:

1. The remembered catalog is descriptive/private and strictly below
   executable authority.
2. The schema is closed, versioned, size-bounded, and rejects unknown or
   ambiguous input.
3. Location hints are absent by default and persisted only after explicit
   user opt-in; their at-rest and presentation boundary is documented.
4. No filesystem, `.git`, Git, provider, runtime, Tool, or authority proof is
   persisted or inferred from a hint.
5. Startup is remembered-but-inert and restores zero repository authority.
6. Explicit fresh admission is the only path from a candidate to a member;
   explicit activation is the only path to active composition.
7. Changed, moved, replaced, unavailable, aliased, nested, and unsupported
   candidates have bounded fail-closed outcomes with no search or replay.
8. Atomic write, synchronization, corruption quarantine, and unsupported
   version behavior preserve the old record and do not silently reset state.
9. Privacy tests cover storage, errors, activity, model/provider surfaces,
   transcript, Effective Authority, and the dedicated human UI.
10. The ADR explicitly retains the existing Tool, policy, currentness,
    HostExplicit, Commit, Stage/Unstage, provider, runtime, and Codex
    boundaries.

If any implementation proposal requires persisted identity proof, automatic
startup validation/admission, authority restoration, path search, model-facing
repository selection, cross-repository composition, credential persistence,
or replay/recovery of effects, Task 330 must stop and request a separate
authority/security decision.

## 12. Explicit non-goals and handoff

Task 329 does not authorize:

- a production file, Rust type, frontend DTO, command, migration, or startup
  reconciliation implementation;
- active member removal or workspace-wide filesystem authority;
- automatic repository re-admission, active restoration, provider reconnect,
  Tool advertisement, or runtime/session restoration;
- persisted filesystem/Git identity, currentness, credentials, provider
  metadata, Tool schema/input/output, tickets, preparations, Commit,
  Stage/Unstage, or uncertain-effect state;
- conversation/session continuity or transcript schema changes;
- path search, repository discovery, network/cloud synchronization, custom
  encryption, OS sandboxing, network isolation, rollback, or effect replay;
- new Tool schemas, PermissionLevels, HostExplicit eligibility, dependencies,
  Cargo changes, release changes, or Codex baseline changes; or
- modification of generated permission files or the amended Task 329
  starting checkpoint.

The recommended handoff is therefore:

```text
Task 329: research contract complete
  -> Task 330: additive ADR and contract decision
  -> only after acceptance: Task 331 implementation
```

No later task starts automatically from this document.

## 13. Source references

- `docs/plans/2026-09-15-task-328-v0.27-scope-and-authority-roadmap.md`,
  especially the Candidate B contract, storage/privacy requirements,
  milestone definition, and Task 329-335 sequence.
- `docs/adr/0027-workspace-repository-identity-authority-composition.md`,
  especially workspace membership, fresh admission, active composition,
  currentness, and nonpersistent HostExplicit boundaries.
- `docs/ARCHITECTURE.md`, current v0.26 multi-repository and inert Trusted
  Profile persistence sections.
- `docs/SECURITY.md`, current v0.26 membership, privacy, ticket, activity,
  and nonpersistence guarantees.
- `crates/rah-desktop/src/repository_membership.rs`, process-local member and
  active-selection state.
- `crates/rah-desktop/src/desktop_preferences.rs`, existing closed inactive
  preference and atomic local-write precedent.
- `crates/rah-desktop/src/conversation_persistence.rs`, separate transcript
  storage owner and namespace contract.
