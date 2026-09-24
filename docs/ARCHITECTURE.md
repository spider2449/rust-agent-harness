# RAH v0.31.0 Architecture - released

The release theme is **Bounded Repository Discovery / Search**. `repo.search`
extends the existing repository observation family without creating a parallel
search authority or generic Git layer.

```text
host-selected active repository identity
        |
        v
existing repository observer envelope
        |
        v
fixed Git tracked inventory
        |
        v
bounded host-side literal matching
        |
        v
repository-relative paths / line numbers
```

The Tool has closed `path` and `text` modes. It searches tracked, currently
present, ordinary regular files in the selected active worktree only. Git is
used only for the fixed inventory command
`git --no-pager ls-files --cached --deduplicate -z --full-name --`; query,
prefix, matching, and all bounds remain host-owned Rust behavior. Text mode
reads current selected-worktree bytes. Results are deterministic, bounded,
best-effort observations with literal case-sensitive matching and no snippets.

An active linked worktree A may search A's tracked worktree files, but cannot
search main or linked B contents. The shared common Git directory does not
create sibling authority, and no Tool input supplies a repository, worktree,
member, or root selector. There is no union ToolRegistry, background index,
filesystem walker, external search executable, or mutation path.

`repo.search` is classified as `ReadOnly` / `RepositoryObservation`, uses the
existing repository-bound `Execute` gate, and is not HostExplicit. The exact
HostExplicit set remains the existing 11 names; `repo.create-directory` remains
ineligible.

Task 368 recorded the independent audit as **PASS WITH NARROW HARDENING**.
Task 369 recorded Windows live certification as **PASS WITH EXPLICIT CODEX
NONCLAIM**. See the v0.31 release gate for certification evidence and
nonclaims.

## Historical RAH v0.30.0 Architecture - released

The release theme was **Linked Git Worktree Repository Support**. ADR 0029 adds
one closed linked-worktree identity form under ADR 0027's existing repository
membership and authority-composition boundary. It adds no executable authority
category.

```text
human-selected worktree root
        |
        v
RepositoryAdmissionIdentity
        |
        +-- ordinary main layout
        |
        +-- validated linked layout
              private Git state
                    |
                    +---- validated relationship ----> common Git state
                                                        |
                                                        v
                                          relationship/dependency context
                                          NOT sibling authority
```

An ordinary main worktree has a real directory at `root/.git`. A supported
linked worktree has a bounded `.git` gitfile resolving to private worktree Git
state, a standard `commondir` relation to shared common state, a backlink to
the selected root's `.git`, and exactly one matching native registration.
Absolute and supported relative link spellings are accepted only when the
complete relationship, filesystem identities, and fixed read-only Git probes
agree. Arbitrary gitfiles are not accepted.

The identity model keeps the following distinctions explicit:

```text
same common Git directory != same repository member
common Git state          != sibling executable authority

one admitted member selected active
        -> one active-only ToolRegistry
        -> selected HEAD/index/currentness
```

Main, linked A, and linked B may share common Git state while remaining
distinct members. There are zero or one active members, never a union
ToolRegistry or parallel active repositories. Activation revalidates identity
and currentness before publishing fresh active-only composition. Close withdraws
composition; inactive removal changes only process-local membership and does
not invoke Git worktree lifecycle commands.

Repository observation and content authoring use only the selected worktree
root. Stage and Unstage use only its private index. Reviewed Commit binds the
selected identity, private index, selected HEAD, attached branch, and expected
selected branch OID. Currentness is capability-specific: unrelated sibling
common-state changes do not globally stale every preparation, while movement of
the selected branch ref does stale its reviewed Commit. No common-directory
generation or sibling authority is introduced.

RAH recognizes an existing valid worktree identity. It does not create,
remove, prune, repair, move, lock, or unlock Git worktrees. Restart restores no
executable repository membership or authority; remembered worktrees remain
descriptive candidates under ADR 0028.

## Authority and decision status

The authority classification is **NARROW EXTENSION**, not a new authority
category. HostExplicit remains exactly 11. No PermissionLevel, Tool field,
Stage/Unstage authority, Commit authority, generic Git/filesystem authority,
provider authority, model routing authority, or persistence/schema authority
was added. ADR 0027 remains authoritative; ADR 0029 is Accepted and adds only
the closed identity form. ADR 0028 and existing mutation/Commit ADRs remain
unchanged.

Task 360 provided deterministic security/currentness hardening and Task 361
provided Windows host-driven certification with explicit nonclaims. GUI
automation, model-selected worktree routing, Codex inference, Linux/macOS live
parity, race-free TOCTOU, OS sandboxing, network isolation, rollback/replay,
machine-wide external-Git locking, submodules, separate-git-dir layouts, and
worktree lifecycle authority remain outside the claim.

# Historical RAH v0.29.0 Architecture - released

This section records the released v0.29 architecture for **Explicit Active
Repository Close**. Close exposes ADR 0027's existing zero-active state
as an explicit human Desktop workflow; it adds no repository authority.

```text
active repository A
       |
       | explicit human Close
       | expected member + repository generation guard
       v
lifecycle/currentness checks
       |
       v
withdraw safe repository-bound executable state
       |
       v
repository generation advances once
       |
       v
active member = none
       |
       v
A remains admitted / inactive
```

The active-member ID and repository generation frozen by confirmation are
equality/currentness guards, not authority. Backend validation rejects a
stale request; if B becomes active after the user confirmed A, the request
returns `active_changed` and leaves B active. It is never reinterpreted as a
request to close the current member.

Membership is distinct from executable authority. There is one or zero active
repositories. A successful Close preserves membership and its generation,
advances repository generation exactly once with checked arithmetic, removes
the active `DesktopRepository`, registry and reviewed Commit authority, and
publishes no active member. Repeated or rejected Close does not churn
repository generation. Explicit reactivation retains the admitted member but
constructs fresh repository currentness. Workflow observation and selector
publication recheck current repository, generation, and member so stale async
work cannot reinstall state after Close or switch.

Close requires a disconnected runtime and provider lifecycle. It does not
invoke Disconnect. Started or uncertain owners remain owned and block Close;
safe prepared state is invalidated. The transition is not cancellation,
rollback, replay, or compensation. It performs no Git or filesystem operation
and does not write the remembered catalog or completed transcript. In-memory
executable repository conversation context is reset; transcript persistence is
not deleted or automatically resumed. Restart remains free of executable
repository membership and authority.

The Task 351 independent audit hardened terminal conversation publication
ownership, poisoned/incoherent fail-closed handling, checked workflow
observation/action sequences, Unstage reservation coverage, Effective
Authority assertions, and Connect ordering. Task 352 then certified the
Windows production backend on its exact source. This host-driven evidence is
not GUI automation and does not establish model-selected Tool dispatch or
cross-platform live parity. See `RAH_V0.29_RELEASE_GATE.md` and
`RAH_V0.29_LIVE_CERTIFICATION.md` for exact identities and limits.

# Historical RAH v0.28.0 Architecture - released

This section records the released v0.28 architecture for **Explicit Inactive
Repository Member Removal**. It completes the process-local curation loop
defined by ADR 0027 while retaining the descriptive remembered-candidate
boundary defined by ADR 0028.

```text
RememberedWorkspaceStore
        ↓
durable inert candidate catalog
        ↓ restart
Desktop remembered presentation state
        ↓ explicit human fresh admission
WorkspaceMembershipState
        ↓ explicit activation
DesktopRepository
        ↓
fresh active-only ToolRegistry / repository authority
```

The durable `RememberedCandidateId` is distinct from the fresh process-local
`RepositoryMemberId`. A persisted location hint is not repository identity and
is never executable proof. Remembered persistence is outside the executable
authority graph: startup loads only bounded descriptive presentation state,
does not probe candidate paths, and restores zero repository authority.

An explicit human fresh-admission action reruns the current ADR 0027
repository identity and currentness checks. Admission creates an inert
process-local member; separate explicit activation creates fresh active-only
composition. Automatic re-admission and automatic activation remain forbidden.

The inactive-member removal lifecycle is:

```text
WorkspaceMembershipState
        |
        | inactive RepositoryMemberId
        v
explicit host Remove
        |
        v
membership_coordination
        -> lifecycle_coordination
        -> current inactive/nonbusy proof
        |
        v
process-local member removed
```

Removal accepts only the current opaque `RepositoryMemberId` for an inactive
member. It is serialized through membership coordination and lifecycle
coordination, proves current inactive/nonbusy state, and removes only that
member. Active composition remains singular and is not rebuilt: removing B from
`members = [A, B]`, `active = A` leaves `members = [A]` and active A unchanged.
Removing active A is rejected. A later explicit admission of the same
repository receives a fresh member ID and remains inert until activation.

Removal is process-local lifecycle management, not a filesystem or Git
operation. It does not write `remembered-workspace.json`, delete repository
files, mutate `.git`, the index, HEAD, refs, or history, or perform automatic
commit, activation, rollback, replay, cancellation, or compensation.

ADR 0027 remains authoritative for process-local membership, fresh repository
identity, one-active composition, switching, and repository-bound authority.
ADR 0028 is additive and owns only remembered candidates, storage/privacy,
restart inertness, fresh re-admission, and descriptive catalog mutation.

The exact production HostExplicit set remains 11 names:

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

`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools,
fixtures/diagnostics, and unknown/provider-defined Tools remain ineligible.
Stage/Unstage remain separate host index authorities and Commit remains a
separate reviewed repository authority. There is no union ToolRegistry or
parallel active repository authority.

The v0.28 Windows evidence is host-driven production-backend certification,
not GUI certification. Model-selected dynamic Tool dispatch remains
unestablished under the approved GPT-5.6-terra live gate.

# RAH v0.26.0 Architecture — released

This document records the released v0.26 architecture. The current immutable
release is `v0.26.0`, with release source
`8b22b18739a54b752ce11e791fe380e522c10049`; the prior immutable release is
`v0.25.0`. The v0.26 annotated tag is `v0.26.0`, its tag object is
`f60ee2465c1819b59b5671f20387b74734ace019`, and its peeled target is the
release source. Task 325 prepared the source, Task 326 published it without a
repository commit, and Task 327 is a later documentation-only descendant,
not the v0.26.0 release source.

## Explicit multi-repository membership and active-only switching

ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary —
defines the v0.26 product boundary:

```text
trusted/human repository admission
  -> process-local WorkspaceMembershipState
  -> inert admitted members
  -> one active RepositoryMemberId
  -> fresh DesktopRepository
  -> fresh active-only ToolRegistry/composition
  -> repository-bound workflow/authority
```

Workspace membership is descriptive and organizational, not filesystem or Git
authority. An inactive member retains no executable repository objects. The
member selector is opaque and process-local; path spelling alone is not
repository identity. Fresh activation revalidates private repository identity.
Nested or alias-duplicate admission fails closed, and nested `.git` ancestry
remains independently blocked for repository-bound path capabilities.

There is one active repository or zero during setup and transition. There is
no union registry and no model/provider repository selection. A successful
switch withdraws the old executable composition and builds a fresh one for the
new active member.

Currentness is separated across membership/lifecycle coordination, repository
generation, model generation, profile generation, connection generation,
Commit identity generation, Stage/Unstage reservation currentness, and
repository-bound HostExplicit tickets/actions. This keeps workflow effects,
runtime publication, and authorization bound to the active repository and its
current lifecycle.

Membership is process-local in v0.26. Persistence and member removal are
intentionally absent. Restart therefore restores no repository membership
authority.

The exact HostExplicit set remains 11 names:

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

Stage/Unstage remain separate host index authority, not HostExplicit. Their
actions are repository/currentness bound; an old A action does not revive after
A→B or A→B→A. Commit remains separate repository-bound reviewed authority and
`repo.commit` is not HostExplicit.

Trusted Profile remains global host-owned composition. MCP and Process Plugin
providers do not select repositories. The Windows evidence was host-driven
with MCP 0 and Process Plugin 0; it does not establish provider-specific live
multi-repository certification.

`MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED GPT-5.6-TERRA LIVE GATE.`
The host-driven certification proves the host-owned authority/effect plane and
real Codex process lifecycle, not current model selection of an RAH dynamic
Tool.

## Historical v0.25.0 Architecture — released

This document records the released v0.25.0 architecture.

RAH v0.25.0 is **RELEASED**.

This is a historical immutable release; `v0.26.0` followed it.

The v0.25.0 release source is:
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.

The prior immutable release is `v0.24.0`.

Task 305 prepared the immutable release source. Task 306 later performed
publication with no repository commit. Task 307 is a later documentation-only
descendant cleanup and is not the v0.25.0 release source.

## v0.25 reviewed HostExplicit file rename/move

The v0.25 release theme is **HostExplicit Reviewed File Rename/Move
(`repo.rename-file`)**. The established layering remains:

```text
Model/Runtime
 -> AgentRuntime
 -> ToolRegistry
 -> host-owned Tool authority
```

The reviewed human route is deliberately capability-specific:

```text
typed human {source_path, destination_path}
 -> zero-effect Prepare -> complete bounded review
 -> opaque single-use ticket -> ticket-only Confirm
 -> currentness / exact ToolDefinition / permission / D2
 -> reviewed Commit authorization invalidation before Started
 -> exactly one authorized_tool_dispatch
 -> current ToolRegistry -> existing repo.rename-file
 -> ADR 0018 policy -> independent reviewed post-effect proof
 -> status-only activity
```

ADR 0018 remains the ordinary rename/move mutation authority. ADR 0021 remains
the generic HostExplicit coordinator/currentness/ticket/D2 boundary. ADR 0026
remains the capability-specific reviewed authority. The frontend and Tauri are
presentation and typed-input surfaces; they do not own repository selection,
authority, native rename, or permission escalation.

The exact production HostExplicit set is 11 Tools:

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

`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools,
fixture/diagnostic Tools, and unknown/provider-defined Tools remain ineligible.
Admission is exact and host-owned; it is not wildcard, prefix,
effect-category, permission-derived, provider-derived, model-derived, or
frontend-derived.

The reviewed preparer binds one existing clean HEAD-tracked regular source
file, same-repository paths, source content no larger than 65,536 raw bytes,
strict UTF-8, no NUL, supported mode, link count 1, exact HEAD/index/worktree
equality, an existing destination parent, and destination absence from the
worktree, HEAD, and every index stage. The complete review is backend-derived
and never truncated. The route produces one unstaged worktree rename/move and
does not rewrite content or references, create directories, overwrite, Stage,
Unstage, Commit, mutate refs/history, or retry/replay/rollback/compensate.

The ordinary Tool remains distinct with its four public fields,
`PermissionLevel::Execute`, five statuses, and at-most-one no-replace native
rename attempt. The reviewed route does not add a second rename implementation
or bypass the current ToolRegistry.

## Task 303 Windows Git-equivalence correction

The final v0.25 architecture includes the Task 303 correction to ordinary
ADR 0018 destination proof. Bounded HEAD tree discovery and bounded
case-insensitive index discovery are narrowing observations; strict Git
candidate parsing and host-owned filesystem path-equivalence comparison are
the collision authority. Filesystem-equivalent tracked/index paths are
rejected even when spelling differs, including tracked-but-missing HEAD,
index-only, intent-to-add, and conflict/unmerged cases, before native effect
with zero native attempts.

The corrected Task 303 Windows certification is carried forward because later
Task 304 changes were documentation-only. It is connected-current host-driven
evidence, not model-selected rename certification and not Linux/macOS live
parity.

## Release state

Task 304 recorded **Verdict A - READY FOR RELEASE PREPARATION** at baseline
`b3532f52b79be9575f3f9d8e818efa2096da4e12`. The publication facts are
recorded above; the architecture itself is unchanged.

---

# Historical v0.24.0 Architecture - released

This document describes the released v0.24.0 architecture.

RAH v0.24.0 is **RELEASED**.

This is a historical immutable release; `v0.25.0` followed it.

The immutable v0.24.0 release source is:
`2d4cf0e4d89a461f54250ff81ad9808929d854f0`.

The prior immutable release is `v0.23.0`, whose release source is
`05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.

Task 287 stopped before publication. Task 288 later performed publication with
no commit. The later Task 289 documentation cleanup commit is not the v0.24.0
release source.

The later Task 278 documentation cleanup commit is not the v0.23.0 release
source.

v0.20.0 preceded v0.21.0.

v0.18.0 preceded v0.19.0.

## v0.24 reviewed HostExplicit file deletion

The v0.24 release theme is **HostExplicit Reviewed File Deletion
(`repo.delete-file`)**. It adds no new generic filesystem boundary. ADR 0017
remains the sole underlying repository file-deletion mutation authority, ADR
0021 remains the generic HostExplicit coordinator/currentness/ticket/D2/
provenance boundary, and ADR 0025 remains the capability-specific reviewed
human deletion HostExplicit boundary.

The connected-current Desktop flow is:

```text
typed human {path}
 -> zero-effect deletion Prepare
 -> complete bounded backend-derived destructive review
 -> opaque process-local single-use ticket
 -> ticket-only Confirm
 -> connected-current/currentness checks
 -> retained deletion-preparer revalidation
 -> exact ToolDefinition / permission membership
 -> D2
 -> reviewed Commit authorization invalidation
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> current ToolRegistry
 -> existing repo.delete-file
 -> ADR 0017 RepositoryFileDeletionPolicy
 -> strict five-status parsing
 -> independent reviewed-route effect proof
 -> status-only terminal HostActivity
 -> descriptive repository refresh
```

The shared `RepositoryDeleteFilePreparer` performs zero-effect preparation and
retains the exact bounded preimage, FileIdentity, link count, parent identity,
Git/index/HEAD state, canonical Tool input, exact ToolDefinition and
permission membership, review identity, and currentness-bound private state.
Confirm revalidates that retained preparation immediately before D2 and
effectful dispatch. The route reaches the existing Tool only through
`authorized_tool_dispatch` and the current `ToolRegistry`; Desktop has no
direct native deletion path. The frontend remains presentation-only and sends
only the typed human path and ticket-only Confirm/Cancel values.

The exact ten production HostExplicit Tools are:

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

`repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools, Process
Plugin Tools, fixture/diagnostic Tools, and unknown Tools remain ineligible.
Eligibility is an exact host-owned set, not wildcard, prefix,
effect-category, permission-derived, provider-derived, model-derived, or
frontend-derived admission.

The reviewed request is closed to `{path}`. The canonical serialized request
is at most 8192 bytes; the logical path is 1..=1024 UTF-8 bytes; the reviewed
source is 0..=65536 raw bytes, strict UTF-8, with NUL rejected; the complete
serialized review is at most 262144 bytes; and retained private preparation is
at most 524288 bytes. The complete deterministic escaped source, including
byte length, SHA-256, BOM/newline/content facts, HEAD/blob and index
relationships, expected effect, Git meaning, non-effects, and destructive
warnings, is the review surface. It is not a preview and may not be
truncated. Exact raw bytes remain private preparation state.

The reviewed target is exactly one existing regular repository file that is
current-HEAD tracked through one normal stage-0 entry, equal in worktree bytes
and index state to HEAD, mode 100644 or 100755, bound to its captured
FileIdentity with link count 1, in a supported ordinary repository state,
strict UTF-8, NUL-free, and no larger than 64 KiB. A verified effect means one
reviewed worktree file is absent and one unstaged Git deletion is present. It
does not Stage, Unstage, Commit, mutate the index, HEAD, branch, refs, or
history, and it does not rename, move, restore, clean up, or recursively
delete.

This reviewed narrowing does not alter the ordinary ADR 0017 Tool. Its closed
input remains `path`, `expected_file_sha256`, and
`expected_file_byte_length`; its ordinary file bound remains 1 MiB, and binary
ordinary deletion remains allowed under ADR 0017.

The five deletion statuses remain exactly `deleted_verified`,
`known_no_effect`, `invalid_input`, `precondition_failed`, and `uncertain`.
`deleted_verified` requires strict valid underlying Tool output plus
independent confirmed absence, retained parent identity, no same-name or
case-equivalent replacement, and unchanged protected Git/index/HEAD/ref
state. `known_no_effect` requires strict valid output plus independent exact
original FileIdentity/link/bytes/hash/length and protected Git/index proof.
Malformed or contradictory output and post-Started dispatch/runtime failure
remain `uncertain` when safe proof is unavailable.

Windows accepts one native `DeleteFileW` attempt followed by one immediate
reviewed post-attempt proof pass. There is no polling, sleep/recheck loop,
second delete, retry, replay, cleanup, restore, or recovery. `DeleteFileW`
success is the filesystem effect commit point, not proof of final absence.
The architecture claims neither race-free TOCTOU, OS sandboxing, network
isolation, nor rollback; process supervision is not an OS sandbox.

## Historical v0.23 ADR 0024 reviewed HostExplicit new-file authoring path

The v0.23 milestone is **HostExplicit Reviewed New-File Authoring** through
the existing `repo.create-file` Tool. The connected-current Desktop human
route is:

```text
typed human {path, content}
 -> zero-effect shared Prepare
 -> complete bounded backend-derived review
 -> opaque single-use ticket
 -> ticket-only Confirm
 -> connected-current / exact-definition / permission / preparer checks
 -> shared creation revalidation
 -> D2
 -> reviewed Commit authorization invalidation
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> current ToolRegistry
 -> existing repo.create-file
 -> ADR 0013 RepositoryFileCreationPolicy
 -> strict result classification
 -> descriptive repository refresh
```

ADR 0013 remains the sole underlying file-creation mutation authority. ADR
0024 owns only this capability-specific reviewed HostExplicit route. ADR 0021
remains the generic HostExplicit coordinator, connected-current/currentness,
D2, ticket, lifecycle, and provenance boundary. HostExplicit is not generic
filesystem authority; the frontend is presentation and typed input only, and
model output or provider metadata is never authorization.

The request is closed to exactly `{path: String, content: String}` and unknown
fields fail closed. The bounds are exactly one file; path 1..=1024 UTF-8 bytes;
content 0..=262144 UTF-8 bytes; NUL rejected; canonical serialized request at
most 327680 bytes; and complete serialized review at most 262144 bytes. Empty
content is allowed. Content remains exact: no BOM or newline transformation,
Unicode normalization, templating, append, overwrite, or automatic final
newline. A complete mutation-relevant review is required; if it cannot fit,
preparation fails before ticket issuance, with no truncated or ellipsized
review authorizing mutation.

The existing parent must already exist and pass root, parent, containment,
identity, and non-link/non-reparse checks. The target must be absent from the
worktree, HEAD, and every index stage including intent-to-add. Ignored targets,
submodules, unsupported sparse-checkout state, Windows reserved/device/ADS/
UNC/verbatim names, reviewed trailing-dot/space components, and symlink,
junction, or reparse traversal fail closed. No parent directory is created.
The native commit point is direct exclusive name acquisition (`O_CREAT |
O_EXCL` intent on Unix; `CREATE_NEW` / `FILE_CREATE` intent on Windows).
Complete writing is not an atomic all-or-nothing transaction.

The exact nine HostExplicit Tools are:

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
```

`repo.delete-file`, `repo.rename-file`, `repo.create-directory`, `repo.commit`,
MCP Tools, Process Plugin Tools, fixture Tools, and unknown Tools remain
ineligible. There is no wildcard, category, or provider HostExplicit route.

The exact ADR 0013 results are `ok`, `invalid_target`,
`precondition_failed`, `create_failed_known`, `write_failed_known`, and
`uncertain`. `create_failed_known` requires bounded post-observation proving
no RAH creation effect. `write_failed_known` proves exclusive creation and may
leave an attributable empty or partial file. `uncertain` preserves unknown
absent, empty, partial, complete, or replaced state. No result is upgraded from
malformed or contradictory output.

Prepare is zero-effect. The opaque ticket is process-local, in-memory,
capability-specific, single-use, exact-preparation/currentness-bound, and
valid for an inclusive five-minute TTL; it is not serializable as durable
authority and has no persistence, resume, replay, or replacement path. Confirm
and Cancel receive only the ticket ID. Generic activity uses a distinct
non-authority activity ID with `ticket_id != activity_id`; it is not derived
from the ticket and cannot Confirm or Cancel. Generic activity, persistence,
and logging exclude the ticket, complete source/content or sentinel,
source-bearing complete review, raw ToolInput/ToolOutput, native paths and
parent identity, and private object identities. Successful generic terminal
output is status-only.

Effectful creation invalidates stale repository-bound reviewed Commit
authorization before HostExplicit Started. Refresh is descriptive only. The
route never retries, replays, deletes, rolls back, restores, compensates,
Stages, or Commits. `write_failed_known` may retain a partial file and
`uncertain` may retain an unknown external effect; timeout, cancellation, or
disconnect is not rollback. No race-free TOCTOU, process-supervision-as-OS-
sandbox, network-isolation, or Linux/macOS production live-parity claim is
made.

Task 274's accepted Windows 10 connected-current host evidence is carried
forward without a live rerun for this preparation. It is human/host initiated,
not model-selected; a connected process and zero model lifecycle counts do not
certify a model turn. The optional live Cancel-before-start subcase was not
rerun; deterministic cancellation evidence was accepted by Task 275.

## ADR 0023 reviewed HostExplicit multi-file authoring path

The v0.22 milestone is **HostExplicit Reviewed Multi-File Edit Authoring**.
The connected-current Desktop human route is:

```text
typed human request
 -> HostExplicit zero-effect Prepare
 -> complete backend-derived ordered review
 -> opaque ticket-only Confirm
 -> shared revalidation / D2
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing repo.edit-files / ADR 0014
 -> strict result classification / descriptive repository refresh
```

The request is closed and typed. It is bounded to 1–4 existing clean
HEAD-tracked regular strict-UTF-8 files, 1–16 exact literal replacements per
target, and 64 replacements total. The host derives exact original-snapshot
matching, preimages, postimages, hashes, lengths, and canonical ascending
UTF-8-byte-order paths. Caller input order is not execution order. Complete
mutation-relevant review is required; `review_too_large` fails closed.

ADR 0014's private `RepositoryMultiFileMutationPolicy` remains the underlying
mutation authority. ADR 0023 defines only this capability-specific reviewed
HostExplicit binding. ADR 0021 remains the general HostExplicit
coordinator/currentness/D2 boundary. HostExplicit reaches the existing
`repo.edit-files` Tool through `authorized_tool_dispatch` and `ToolRegistry`; it
does not bypass them or create generic repository authority.

The exact HostExplicit allowlist is:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
```

`repo.create-file`, `repo.delete-file`, `repo.rename-file`,
`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools, and
unknown Tools remain ineligible. Eligibility is an exact eight-name list, not
a wildcard or category rule. Permission classification, Trusted Profile or
provider metadata, frontend state, and human review do not create mutation
authority; model output is never authorization.

Prepare has zero Tool and native mutation effects. The ticket is process-local,
in-memory, opaque, single-use, exact-change/currentness-bound, and valid for an
inclusive five-minute TTL. Confirm and Cancel receive only `{ ticketId }`.
Generic HostExplicit activity uses a separately generated RAH activity ID. It
is not the ticket, is not derived from the ticket, cannot Confirm or Cancel,
and generic activity excludes the ticket and source-bearing review content.
Task 263 Attempt 1 recorded the historical ticket-under-`invocationId` leak
before Confirm with zero effect; Attempt 2 corrected the separation and passed
in a fresh Windows repository.

The operation is explicitly non-atomic and preserves the six ADR 0014 result
classes: `ok`, `invalid_target`, `precondition_failed`,
`failed_known_no_effect`, `partial_effect`, and `uncertain`. Verified committed
prefixes and uncertainty are reported conservatively. There is no retry,
replay, prefix continuation, rollback, restore-preimage, transaction claim,
automatic Stage, or automatic Commit. `repo.patch` and `repo.edit-files`
invalidate reviewed Commit authorization at Started; refresh is descriptive
and does not grant Commit authority. The narrow empty-index refresh fallback
preserves descriptive status/diff observation without fabricating Commit
authority.

## ADR 0022 reviewed HostExplicit worktree-authoring path

The v0.21 milestone is HostExplicit Reviewed Single-File Patch Authoring. The
product flow is:

```text
inspect
 -> typed human repo.patch Prepare
 -> exact bounded review
 -> ticket-only Confirm
 -> existing repo.patch
 -> inspect diff
 -> existing Stage / Unstage
 -> existing reviewed Commit
```

The exact HostExplicit eligibility is seven Tools: `fs.read`, `repo.file-info`,
`repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch`, and
`repo.patch`. The v0.21 authoring form is H1 only: `path`,
`expectedOldText`, and `replacementText`. The frontend renders those typed
fields and does not choose eligibility or authority.

The host derives SHA-256 and byte-length bindings and the canonical
`repo.patch` ToolInput. Shared non-effectful `RepositoryPatchPreparer` creates
the exact escaped R4 review and opaque process-local single-use five-minute
ticket. Confirm receives only the ticket ID, revalidates the preparation, runs
D2 preflight before `Started`, then uses `authorized_tool_dispatch` to reach
the existing `repo.patch` Tool and its ADR 0012 policy. Prepare performs zero
Tool executions and zero replacements; the effectful path permits exactly one
Tool execution and one native replacement attempt.

ADR 0012 remains the sole underlying worktree-content mutation authority. ADR
0021 remains the general HostExplicit dispatch, currentness, D2, coordinator,
and provenance boundary. ADR 0022 adds only the reviewed workflow around that
existing capability; it creates no generic write authority. Stage, Unstage, and
reviewed Commit remain separate existing actions. `repo.create-file`,
`repo.edit-files`, `repo.delete-file`, `repo.rename-file`,
`repo.create-directory`, `repo.commit`, MCP Tools, and Process Plugin Tools are
not HostExplicit eligible.

Effectful start clears old reviewed-commit authorization. Repository refresh
may produce a new `ReadyToAuthorize` review, but never automatically
authorizes it. Changing content alone does not increment
`repository_generation`. Strict result classification and conservative
post-start uncertainty prohibit retry, replay, rollback, or restore-preimage.

## ADR 0021 explicit Host Tool invocation path

RAH v0.20 adds an explicit Desktop Host Tool invocation workflow for a closed
first-party Tool set. The authority invariant is:

```text
host explicit dispatch != runtime/model dynamic dispatch != capability authorization
```

The HostExplicit route is:

```text
Human Desktop action
 -> backend typed operation
 -> connected-current Host composition
 -> backend eligibility
 -> authorize_tool_dispatch
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing capability Tool/policy
```

The runtime/model route remains:

```text
Model/Runtime
 -> Codex route ownership
 -> shared D2
 -> ToolRegistry
 -> Tool
```

Both routes share D2, but route-specific checks and provenance remain separate.
HostExplicit creates no repository, filesystem, branch/ref, commit, provider,
process, network, or other capability authority. `PermissionLevel` is only a
dispatch category; `Execute` is not generic repository authority. Frontend
state, Tool visibility, Effective Authority, and human confirmation are
observational or workflow inputs, not authorization.

D2 binds the call name to the expected Tool, re-resolves the current Tool from
the current registry, requires exact complete `ToolDefinition` equality
(name, description, input schema, and permission), and requires the current
permission to be explicitly present in the host allowed policy. There is no
permission hierarchy. Rejection executes zero Tools; admission executes
exactly one Tool with no retry. D2 does not own eligibility, authority,
lifecycle/provenance, result interpretation, replay, or rollback.

The Codex bridge preserves thread and active-turn ownership, its private alias,
replay/deduplication, cancellation, response translation, and real
`AgentEvent` lifecycle. It authorizes before `ToolStarted` and revalidates at
execution. A captured permission change is a stale definition even when the
replacement permission would otherwise be allowed. This hardens dispatch
admission; it does not claim to make model Tool selection more reliable.

HostExplicit is connected-current only. The current registry, expected
definitions, shared allowed permission policy, repository/model/profile/
connection generations, and selected repository/context remain bound. Stale
or disconnected state fails closed; there is no silent reconnect, recompose,
provider activation, Trusted Profile restore, or disconnected execution route.

The first-release eligibility is exactly:

- `fs.read`
- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`
- `repo.create-branch`

The typed forms are `path` for `fs.read` and `repo.file-info`, `{}` for the
two diff/status observers, `name` for branch Prepare, and ticket ID only for
branch Confirm. There is no generic ToolName/arbitrary JSON route. Branch
Prepare has no Tool execution or Git effect; after sanitized review, Confirm
consumes an opaque process-local in-memory single-use Tool/input/composition-
bound ticket with a five-minute TTL, revalidates currentness, then enters D2
and existing ADR 0020 authority. It does not auto-reprepare or retry.

The coordinator permits at most one HostExplicit invocation, excludes host work
during a model turn and model work during `HostPrepared` or `HostRunning`, and
has no wait queue or automatic retry. Read-only host actions participate. The
conceptual states are `Idle`, `ModelTurn`, `HostPrepared`, and `HostRunning`.
HostExplicit provenance is Desktop-private `host_explicit`, not model
`ToolRequested`, `ToolStarted`, or `ToolFinished`; no `AgentEvent` schema
change or forged model lifecycle was introduced.

Before start, prepared branch work may be cancelled with known no Tool effect.
After start, there is no active HostExplicit abort operation; execution is
owned to terminal handling where possible. UI dismissal is not abort. There is
no retry, replay, compensation, rollback, ticket persistence, capability
persistence, automatic resume, conversation continuation, chat message, or
ToolOutput injection into model context. External provider Tools,
`repo.commit`, and worktree-authoring Tools are deferred from HostExplicit.

## ADR 0020 bounded local branch creation

The v0.19 first-party `repo.create-branch` capability adds one narrow,
host-owned local branch creation plane:

```text
Model / Runtime
        -> ToolRegistry
        -> repo.create-branch
        -> host-owned RepositoryBranchCreationAuthority
        -> private RepositoryBranchCreationPolicy
        -> fixed native Git create-only CAS
```

The model supplies only `{"name":"<validated-logical-branch-name>"}`. The
host selects the repository and native Git executable, captures the attached
committed `HEAD`, validates the closed ASCII branch-name language and all
repository preconditions, then constructs the fixed `refs/heads/<name>` target
and zero-old-OID expected-absence CAS. The one permitted secondary effect is
the Git-owned reflog entry with the fixed host-controlled message.

Branch creation authority != branch switching != index mutation != worktree
mutation != commit/history authority != generic ref/Git authority. The
capability does not checkout, switch, create-and-switch, delete, rename, force
update, set tracking/upstream, address tags/remotes, or accept arbitrary refs,
OIDs, revisions, argv, hooks, or environment. Dirty, staged, and mixed ordinary
repository states remain admissible because the new ref targets committed
`HEAD`, not index or worktree state.

Desktop owns an optional `RepositoryBranchCreationAuthority` for the selected
repository. `repo.create-branch` is registered only when both the selected
repository and valid host branch authority are present. No repository, a
repository without authority, Execute alone, startup, Trusted Profile state,
provider metadata, or frontend presentation can manufacture it.

The sanitized Effective Authority classification is `repository_mutation`,
`repository_local_branch_creation`, `execute`, `repositoryBound: true`, with
source `repository_host` / `desktop_repository` and frontend label `Local
branch creation`. The frontend presents this backend classification; it does
not authorize or infer authority from the Tool name.

Verified create-only branch creation preserves reviewed commit authorization
and the active branch, `HEAD`, index/tree plane, worktree, tracking state,
conversation namespace, provider composition, and all repository/model/profile/
connection generations. It does not require reconnect or repository refresh.
Malformed, uncertain, or started-but-unfinished results use conservative
currentness handling with bounded refresh and no replay or rollback. The
public outcome taxonomy remains `invalid_input`, `precondition_failed`,
`known_no_effect`, `branch_created_verified`,
`desired_state_observed_after_uncertain_attempt`, and `uncertain`.

The Windows host-driven authority/effect path is certified. Model-selected
`repo.create-branch` dispatch was not observed in two bounded Codex live
attempts and is not certified. The fresh successful branch name and OID were
internally asserted but were not echoed on the successful output path; this is
a documented evidence-capture limitation. Linux live branch certification is
not established.

## Effective Authority observability path

The v0.16 Desktop review is an observation path over existing host-owned
composition and lifecycle state:

```text
Desktop host state
        -> sanitized EffectiveAuthoritySnapshot
        -> authority-observation Tauri command
        -> Effective Authority panel
```

The backend is the sanitization boundary. It derives currentness from the
repository, runtime, model, and Trusted Profile generation tuple plus
publication checks, exposes public Tool
names and closed host-derived classifications, and excludes private aliases
and sensitive provider or repository details. The frontend only renders the
sanitized DTO; it is not an authority or security source.

Configured intent, effective composition, runtime advertisement, and
unconditional execution authorization are distinct states. A visible or
advertised Tool still passes ToolRegistry lookup, host permission and policy
checks, repository/workspace constraints, generation/preconditions, and any
separate reviewed-commit authorization. Stale or reconnect-required inventory
cannot become Current for a new repository or model context.

Effective Authority collection is observational with respect to authority
decisions and executable capability: it does not grant, issue, consume, or
authorize executable authority. Root gathering observes live state and may
reap a HostExplicit preparation whose ticket has already expired and is
unusable, clearing its retained coordinator state before reporting
availability. That bounded workflow-bookkeeping cleanup can change the
reported busy/availability presentation; it does not make a valid ticket
unusable or change repository, provider/runtime, or Commit authority. The
`effective_authority` composition step receives closed facts and
deterministically derives the snapshot; it does not perform expiry cleanup or
access live owners.
Inspection and Refresh Authority do not compose, reload, spawn, reconnect,
execute, persist, or make authority decisions. MCP and Process Plugin adapters
remain Tool providers under the existing architecture. The v0.16 release path did not
compose their authority-review presentation through Desktop; the v0.17
provider-only overlay now composes admitted local providers at Connect and
publishes their host-derived descriptors through the same observation path.

## Inert Trusted Profile persistence and explicit restore

v0.18 persists one host-owned Trusted Profile source-path preference in the
Desktop store. It is desired state, not effective composition or authority:
startup is remembered-not-restored and does not load, validate, compose, spawn,
advertise, or restore a provider. Explicit Restore fresh-loads and statically
validates the current source without spawning. Explicit Connect or reconnect is
the sole provider activation boundary and fresh-loads the source again before
composition. Forget removes only the durable preference and does not alter an
already connected provider composition.

This preference is not model-facing configuration and does not change the
Trusted Profile authority-composition boundary, ToolRegistry dispatch, host
permission decisions, provider admission, or lifecycle ownership. v0.18 adds
no active-provider auto-restore, hot reload, credential persistence, provider
installation, network MCP, or new tool authority.

## Ownership boundaries

RAH public boundaries use only RAH-owned neutral types. `rah-protocol` is the
dependency-bottom crate and contains serializable identifiers, messages, events,
tool descriptions, calls, and outputs. Provider, runtime, MCP, and process-plugin
adapters translate only at their private edges.

`AgentRuntime`, `ModelBackend`, `Tool`, `ToolRegistry`, `SessionStore`, and
`Sandbox` remain independent extension points. No v0.4 work changes their
architecture-defining public contracts.

ADR 0011 establishes the trusted host capability profile as the authority-
composition boundary for existing built-in capabilities and admitted external
providers. It does not change runtime, `Tool`, `ToolRegistry`, or
capability-specific policy contracts.

ADR 0012 establishes a distinct private, host-owned
`RepositoryWorktreeMutationPolicy` for the bounded `repo.patch` capability.
It is deliberately separate from ADR 0010's index-only
`RepositoryMutationPolicy` and ADR 0011's composition-only profile boundary:

```text
worktree content mutation != index mutation != history/ref mutation
```

ADR 0013 establishes a separate private, host-owned
`RepositoryFileCreationPolicy` for `repo.create-file`. It authorizes only one
exclusive creation of one absent UTF-8 file under a host-bound repository; it
does not grant generic `fs.write`, overwrite, directory creation, staging, or
index/history/ref authority. `repo.patch` and `repo.create-file` share the
same per-repository mutation lease, so they cannot independently widen mutation
concurrency or authority.

ADR 0014 adds `repo.edit-files` as a separate host-constructed capability with
private `RepositoryMultiFileMutationPolicy`: it edits one through four existing
clean tracked UTF-8 files in deterministic host order. `Execute` is its outer
permission only; it makes no cross-file transaction, rollback, retry, or replay
claim and classifies bounded partial or uncertain effects. Trusted Profile v1
static validation records only symbolic bindings without
constructing it; effective composition constructs and publishes it through a
fresh registry on complete success. Generic Tool Bridge dispatch remains
capability-agnostic. Codex live certification remains deferred.

ADR 0015 adds one Desktop-private, human/host-selected initial `llama_cpp`
endpoint. Rust validates the closed endpoint structure and synthesizes the
fixed `/v1` base URL; it is not a Tool, generic network surface, credential
store, provider lifecycle manager, or public RAH abstraction. Saved Desktop
model preferences are inactive desired state only. They do not auto-connect,
recreate a runtime or `ToolRegistry`, or restore authority.

ADR 0016 adds `repo.commit` without redesigning those boundaries:

```text
Model / Runtime
  -> AgentRuntime
  -> ToolRegistry
  -> repo.commit
  -> host-owned repository commit policy / native Git
```

Trusted Profile composition selects the fixed host-owned repository, native Git
resource, and identity, then publishes the ordinary `repo.commit` Tool through
a fresh registry. It does not authorize a staged snapshot. The host-only
`RepositoryCommitControl` separately arms one fresh reviewed snapshot; the
model sees only the message schema and the Generic Tool Bridge sees only a
private routing alias. `Execute` is an outer permission gate, not history
authority. The private policy fixes the one normal commit invocation and
revalidates repository, executable, hooks, attached HEAD, branch, compound
index snapshot, and postconditions under the shared RAH repository mutation
lease. This is neither generic Git nor branch/ref authority.

ADR 0017 adds `repo.delete-file` as a separate private, host-owned deletion
authority. It admits one explicitly named repository-relative regular file
only when the clean HEAD-tracked target matches the exact authorized HEAD
blob preimage, including raw bytes, SHA-256, and byte length. It performs one
native worktree deletion attempt and leaves the index unchanged, so the result
is an unstaged deletion. It does not add rename/move, directory or recursive
deletion, arbitrary untracked deletion, generic filesystem or shell authority,
staging, commit, or ref/history/network Git authority. Execute remains only an
outer dispatch permission; model requests, provider metadata, Trusted Profile
composition, Tool definitions, and frontend state cannot manufacture the
separate host authority. The Generic Codex Tool Bridge translates the
canonical public name through a private alias without changing authorization.

ADR 0018 adds `repo.rename-file` as a separate private, host-owned rename/move
authority. `RepositoryFileRenamePolicy` accepts exactly one clean,
HEAD-tracked regular file and an explicit absent destination in the same
repository, for either a same-directory rename or a cross-directory move. The
source SHA-256 and byte length are mandatory request preconditions. The host
revalidates repository, source, destination, HEAD, index, branch, runtime, and
repository-generation identity immediately before one native no-replace
filesystem effect. It does not use `git mv`, copy-delete, generic
`fs.rename`, shell/process fallback, staging, or commit. A possible effect is
`uncertain` and is never replayed. This authority is distinct from creation,
deletion, content mutation, index mutation, and commit/history mutation; the
Generic Codex Tool Bridge and Desktop only expose it after host composition.

ADR 0019 adds `repo.create-directory` as a separate private, host-owned
`RepositoryDirectoryCreationPolicy`. It accepts exactly one explicit
repository-relative destination path and creates exactly one ordinary
directory leaf under an existing parent when the destination is absent. It
does not create parents, ensure an existing directory, create placeholders, or
mutate Git. The host revalidates repository and repository-generation identity
immediately before one handle-relative Windows or descriptor-relative
Unix/Linux effect. A possible effect is never replayed or compensated.

Directory creation shares the repository mutation lease with other RAH-owned
repository filesystem mutations while remaining distinct authority from file
creation, deletion, rename/move, content mutation, index mutation, and
reviewed commit/history mutation. Desktop binds it to the selected repository
and generation; a verified filesystem mutation triggers refresh and reviewed-
commit authorization revocation even when `git status --short` remains clean,
because Git does not track empty directories.

Desktop repository context is host-owned. The selected canonical repository
supplies fixed Git identity, `safe.directory`, and verified runtime CWD at
`thread/start`; no-repository mode uses a neutral app-owned workspace.
Repository-scoped transcript storage uses an opaque SHA-256 namespace rather
than a raw path. Its private SQLite backend is storage only: it is not exposed
as SQL to models or tools and does not grant repository mutation authority.

The v0.14 Desktop workflow retains the v0.13 bounded capabilities and adds
the separately authorized deletion capability without collapsing authority
boundaries. The existing workflow remains: model bounded authoring is followed
by host-owned
Stage / Unstage, host-observed staged review, and an explicit host-owned
reviewed-snapshot authorization before message-only `repo.commit`. The
frontend presents sanitized host state only. `RepositoryCommitReview` remains
opaque and Rust-only; it is neither model data nor frontend authority.

The public `RepositoryWorktreePatchTool` constructor fixes a host-selected Git
executable and repository root. Its closed request schema permits only a
logical relative path, complete-file SHA-256 and byte-length preconditions, one
nonempty literal expected text, and replacement text. The private policy
performs all repository eligibility, identity, replacement, and uncertainty
handling; model requests and `PermissionLevel::Execute` remain insufficient
authority by themselves.

The repository-observation tools are four separate, host-constructed tools:
`repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`. They use
a crate-private fixed-command observer envelope with host-selected executable
and repository identities, fixed cwd/environment, bounds, and the existing RAH
repository lease. `Execute` remains an outer process gate, not generic Git,
filesystem, or mutation authority. This bounded observation work neither
extends ADR 0010 index mutation nor ADR 0012 worktree mutation; ADR 0011 alone
governs trusted-profile composition.

## Trusted capability profile

ADR 0011 defines trusted profile source validation and the explicitly selected
trusted static profile as a host-only authority-composition boundary. The profile
selects existing constructors, symbolic resources, exact external-provider
admission, and explicit permission mappings; it is neither an `AgentRuntime`
nor a model-facing API.

```text
Trusted Host Capability Profile
             |
             v
      effective composition
             |
             v
         ToolRegistry
       /      |       \
 built-in    MCP     Plugin
```

Static validation parses and validates the source/profile/resources without
launching a provider. Explicit effective composition resolves host-owned
resources, launches configured providers where required, admits their exact
declared tools/schemas, and returns a fresh registry only after complete
success. The effective profile owns its provider adapters for the registry's
usable lifetime.

The runtime remains downstream of this boundary. `CodexRuntime` receives a
host-supplied registry through its optional Generic Tool Bridge and cannot
select providers or profile authority.

## Preserved v0.3 capability classification

### Public / host capabilities

The host-owned Execute surface includes `host.cargo.version`, `host.git.status`,
`host.git.stage`, `host.git.unstage`, `repo.create-file`, `repo.edit-files`, and the fixed observers
`repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`, and
  `repo.rename-file`, and `repo.create-directory`. The first
two and the observers are fixed, host-constructed inspection capabilities.
Stage and unstage use the private `RepositoryMutationPolicy` to prove one
authorized index-only effect for one host-selected target. They never grant
generic process, worktree-byte,
history/ref, network, or credential authority.

### Validation fixtures

The hardened Execute deterministic/live fixture (`process.test.echo`) and the
repository-mutation deterministic/live fixture are validation infrastructure.
They establish policy behavior before the public host capabilities are exposed;
they are not production/public capabilities. In particular,
`host.fixture.echo` does not exist.

The Generic Tool Bridge, `fs.read`, MCP adapter, and process-plugin adapter are
also verified v0.3 components. All converge through RAH-owned `Tool`,
`ToolRegistry`, and permission interfaces.

## Current crate topology

Production RAH dependency edges are:

```text
rah-core                                  (no RAH dependencies)
rah-sandbox                               (no RAH dependencies)

rah-protocol                              (dependency bottom)
  ^        ^          ^          ^
  |        |          |          |
model   session      tools     runtime
                     ^  ^         ^
                     |  |         |
             tools-mcp  tools-plugin

rah-tools   -> rah-protocol, rah-sandbox
rah-runtime -> rah-model, rah-protocol, rah-tools
rah-runtime-codex -> rah-protocol, rah-runtime, rah-tools
rah-cli     -> rah-model, rah-protocol, rah-runtime, rah-tools
```

`rah-tools-mcp` and `rah-tools-plugin` each depend on `rah-protocol` and
`rah-tools`. They do not depend on a runtime. `rah-runtime-codex` has no
production dependency on either adapter crate and contains no MCP- or
plugin-specific dispatch. Its manifest uses them only as dev dependencies for
the opt-in examples and cross-boundary tests.

## Tool convergence

Every tool source converges through the profile/composition boundary before
runtime dispatch:

```text
Built-in Tool -----------\
MCP Tool -----------------+-> Tool -> ToolRegistry
Process Plugin Tool ------/
```

The registry is unaware of transport or provider. It stores `Arc<dyn Tool>`,
returns deterministic definition snapshots, rejects duplicate names, and
dispatches parsed `ToolCall` values. Host composition selects adapters, assigns
permissions, and registers their proxies.

`ExternalToolIdentity` is opaque and provider-neutral. The host uses
`ExternalToolPermissionPolicy` to assign a `PermissionLevel` to each discovered
identity. Missing assignments fail closed before registration; server/plugin
metadata cannot grant authority.

## Deterministic runtime

`MinimalTestRuntime` proves the provider-neutral loop using `MockBackend`. Its
default host policy allows only `PermissionLevel::None`; the manifest demo
explicitly adds `Read`, while `FsReadTool` independently enforces its configured
workspace boundary.

## Generic Codex Tool Bridge

`rah-runtime-codex` owns these private layers:

```text
CodexRuntime
 -> optional generic RAH Tool Bridge
 -> session/thread/turn translation
 -> correlated connection actor
 -> private JSON-RPC parsing
 -> stdio transport
 -> owned codex app-server child
```

The executable must report `codex-cli 0.149.0`. The adapter generates the
installed app-server schema locally and verifies the required lifecycle fields;
bridge mode additionally verifies the version-pinned experimental dynamic-tool
contract.

Bridge mode snapshots any host-supplied `ToolRegistry` for a new Codex thread.
It advertises provider-private aliases where RAH tool names are not accepted by
Codex, translates a valid request into the original RAH `ToolCall`, checks the
host's allowed permission levels, dispatches through the registry, emits RAH
tool lifecycle events, and returns the translated result. Dedupe, replay,
cancellation, correlation, and call bounds remain adapter-private.

The bridge does not know whether a registered tool is built-in, MCP-backed, or
process-plugin-backed. Codex-owned shell, file, MCP, web, image, app, and approval
capabilities remain disabled even in bridge mode.

## External process adapters

`rah-tools-mcp` owns the pinned MCP `2025-06-18` stdio handshake, discovery,
request correlation, timeout, cancellation, result conversion, child ownership,
and immutable `mcp.<server>.<tool>` proxies.

Trusted static profile composition is host-only. Its static pass parses closed
symbolic MCP and Process Plugin declarations without launching a provider;
explicit effective composition delegates construction and exact admission to
their hardened adapters, then publishes a fresh `ToolRegistry` only when every
provider has validated. Provider-qualified names and registry duplicate checks
fail closed where names could otherwise collide. The effective profile retains
adapter ownership for as long as its tools are usable.

`rah-tools-plugin` owns RAH process-plugin protocol version `1`, identity and
version validation, host-selected executable identity checks, exact expected
tool/schema admission, bounded NDJSON stdio, resource limits, process
lifecycle, and immutable `plugin.<plugin>.<tool>` proxies. Admission builds
privately and publishes only after the complete provider validates. It is a
focused adapter, not a general plugin manager, installer, marketplace, SDK, or
dynamic-library ABI. It is a trusted-profile provider only through the closed
ADR 0011 `process_plugins` declaration and the host-owned effective composer.

## Conformance and architecture gates

Generic deterministic conformance helpers cover observable `ModelBackend`,
`Tool`, `SessionStore`, and `AgentRuntime` contracts. Adapter tests use local
fixtures and fake Codex transport. Architecture gates prevent Codex Rust
dependencies, provider dependencies in core crates, upward dependencies from
`rah-protocol`, and escaped Codex implementation details. The production
manifest keeps MCP and process-plugin adapters out of `rah-runtime-codex`.
