# Task 340 — RAH v0.28 Inactive Repository Member Removal Contract and Lifecycle Acceptance Matrix

Status: implementation-ready contract

Date: 2026-09-16

This document freezes the v0.28 inactive process-local repository member
removal contract for Task 341. It is a documentation-only Task 340 artifact.
It does not add Rust, frontend behavior, Tauri permissions, Cargo changes,
authority, HostExplicit eligibility, Codex behavior, persistence schema, or an
ADR.

## 1. Baseline and authoritative decisions

The required starting checkpoint was verified before modification:

| Fact | Required value |
| --- | --- |
| Starting `HEAD` | `87de35ece3f29fe8f4c7f39dbd3b734211b1fed2` |
| `origin/master` | `87de35ece3f29fe8f4c7f39dbd3b734211b1fed2` |
| Starting worktree | clean |
| Task 339 exact-head CI | `35066087146` — PASS |
| Current published release | RAH v0.27.0 |
| Workspace | 13 packages, all `0.27.0`, Rust edition 2024 |
| HostExplicit production count | exactly 11 |
| Certified Codex baseline | `codex-cli 0.149.0` |

ADR 0027 already accepts process-local descriptive membership, opaque
`RepositoryMemberId` selectors, one active repository, active-only executable
composition, switching/currentness, and inactive-member removal. ADR 0028
continues to own only durable remembered descriptive candidates. No new ADR is
required. Neither ADR is changed by Task 340.

The current source review confirms these structural facts:

- `WorkspaceMembershipState` stores only inert member data and an optional
  active selector. It does not store a ToolRegistry, provider, runtime,
  Commit authority, Stage/Unstage authority, or HostExplicit queue per member.
- `DesktopAppState` has one active `DesktopRepository`, one repository
  generation, one repository workflow, one index-effect reservation, one
  reviewed Commit capability, one HostExplicit coordinator, one connection,
  one active chat/conversation state, and one provider activation.
- Activation publication is serialized by `membership_coordination` followed
  by `lifecycle_coordination`, with final member and generation checks before
  publication.
- Switching clears prepared HostExplicit state, withdraws Commit/workflow
  state, publishes a fresh active repository, and starts a new in-memory
  conversation context.
- The current membership primitive increments `membership_generation` before
  checking whether removal succeeds. Task 341 must retain
  `WorkspaceMembershipState::remove` as the sole mutation primitive but correct
  that ordering so active, unknown, and stale removal attempts have no state
  or generation effect.

## 2. Product decision and authority boundary

The only v0.28 product action is explicit human/host removal of one existing
inactive process-local member:

```text
closed RepositoryMemberId selector
  -> host parses the bounded selector
  -> current membership resolves the member
  -> target is proved inactive
  -> target-bound lifecycle ownership is proved absent
  -> WorkspaceMembershipState::remove(target) linearizes
  -> removed selector becomes unreachable
  -> sanitized membership presentation is returned
```

Removal changes process-local membership reachability only. It is not a
remembered-candidate operation, active-repository removal, repository deletion,
Git operation, Tool, model action, provider action, or permission operation.

Removal MUST NOT:

- mutate repository files, `.git`, HEAD, refs, the index, history, or Git
  configuration;
- delete, update, reorder, or otherwise mutate `remembered-workspace.json`;
- activate another member, deactivate the active member, switch repositories,
  build a ToolRegistry, destroy the active ToolRegistry, disconnect a runtime,
  shut down a provider, or rebuild active composition;
- revoke unrelated active Commit authority or cancel unrelated active
  Stage/Unstage state;
- alter the active conversation binding or delete/hide durable transcript
  history;
- create, consume, migrate, or expose any authority ticket;
- become a ToolRegistry Tool, MCP Tool, Process Plugin Tool, Codex dynamic
  Tool, or model-facing repository selector; or
- persist executable membership or removal state.

The frontend may display a Remove control and request the operation, but it
does not select authority. The model and provider cannot request it.

## 3. Selector and result contract

### 3.1 Input

The command accepts exactly one bounded string carrying the existing
process-local `RepositoryMemberId` selector representation. It uses the
existing strict parser and current workspace epoch/ordinal semantics.

The request MUST NOT accept a native path, repository root, Git directory,
filesystem identity, remembered candidate ID, Tool name, provider identity,
profile identity, generation tuple, or frontend authority metadata. A selector
is meaningful only in the current process and current membership generation
context.

Malformed input is rejected before any state lock can mutate state. A
syntactically valid selector for a removed member is not recovered through
path, label, remembered-candidate, Git-identity, or recent-history matching.

### 3.2 Closed result/error surface

Task 341 shall expose a small closed Desktop result surface. The success DTO
contains only a success outcome and the existing sanitized
`WorkspaceRepositoryMembershipPresentation`.

| Result | Meaning | State effect |
| --- | --- | --- |
| `removed` | The selected existing inactive member was removed. | One member disappears; membership generation advances once. |
| `selector_invalid` | Selector is malformed, overlong, or outside the accepted syntax. | Zero effect. |
| `member_not_found` | Selector is well-formed but no longer resolves, including a stale or double-remove selector. | Zero effect, including no generation advance. |
| `active_member` | Selector resolves to the current active member. | Zero effect, including no generation advance. |
| `busy` | A target-bound lifecycle owner or transition cannot safely be proven absent/current. | Zero effect. No cancellation, rollback, or retry. |
| existing internal unavailable/poisoned result, if required by current Desktop conventions | The host cannot safely inspect its state. | Zero effect and no sanitized success presentation. |

The implementation may map these categories to existing Desktop error names,
but it must preserve the distinctions. The preferred explicit active error is
`RepositoryMemberActive`; `RepositoryBusy` remains the lifecycle-busy
category. No error contains a native path, `.git` value, filesystem identity,
authority handle, ticket, provider handle, generation tuple, raw Git error, or
remembered-catalog location.

### 3.3 Presentation and activity

Success returns the existing membership presentation shape: opaque `memberId`,
bounded display name, active/inactive status, active member selector, and
membership generation. It does not return the removed member's root or private
identity. The removed selector is absent from the returned list.

No Activity entry is required. If the Desktop chooses to emit one, it MUST be
status-only with the conceptual text:

```text
Repository removed from current workspace
```

Activity MUST NOT contain a native path, filesystem identity, Git directory,
remembered candidate location, ticket, provider data, or source-bearing data.

## 4. Normative lifecycle contract

### 4.1 Successful removal

For `members = [A, B]` and `active = A`, removing B succeeds only after the
host has resolved B in the current membership and proved B inactive. The
linearized postcondition is:

```text
members = [A]
active = A
```

The active `DesktopRepository`, repository root/identity, ToolRegistry and
composition identity, runtime/provider connection, conversation context,
active Commit state, active Stage/Unstage state, and current model/runtime
connection remain unchanged. No active composition rebuild is permitted for
this membership-only change.

For a workspace with no active member, removal of an existing member is still
allowed if no target-bound lifecycle owner exists. For a workspace with one
member, that member is active and therefore cannot be removed by this action.

### 4.2 Active-member rejection

If target equals `active_member`, removal returns `active_member` and has zero
effect. It does not switch, deactivate, disconnect, clear conversation, revoke
authority, remove membership, or delete a remembered candidate. The mandatory
human flow is:

```text
explicitly activate another member
  -> old member is inactive
  -> issue a separate explicit removal request
```

There is no one-click switch-and-remove path.

### 4.3 Lifecycle-busy rule

An inactive target is removable only when no lifecycle owner could still publish
or complete an effect bound to that target. The implementation must derive
this from existing owners and currentness; it must not add a global `busy`
flag.

Under the accepted active-only architecture, all executable repository-bound
state is owned by the active member. Therefore, for a correctly maintained
state, an inactive member cannot own any of the following:

- an activation publication transaction;
- a repository workflow action or refresh that can publish target-bound
  executable state;
- a Stage/Unstage reservation or started/uncertain index effect;
- a usable Commit review/authorization;
- a prepared, started, or uncertain HostExplicit effect;
- a live `DesktopRepository`, ToolRegistry, provider, runtime, or executable
  conversation binding.

Switching away already invalidates active repository actions, Commit state, and
prepared HostExplicit state; switching is itself blocked by an active index
effect, and activation publication has final membership/currentness checks.
These are structural proofs, not cleanup performed by removal.

If an implementation observes a target-bound owner despite those invariants,
it fails closed with `busy`. It must not clear, cancel, replay, compensate,
rollback, or migrate that owner. A Started effect remains Started/uncertain
until its existing lifecycle resolves or refreshes.

Unrelated active-A owners are not target-busy when removing inactive B. They
must remain unchanged. In particular, removal of B MUST NOT reject merely
because A has an active conversation, active Commit state, active Stage/Unstage
state, an active HostExplicit prepared ticket, a HostExplicit effect, or a
connected runtime/provider, provided current architecture proves those owners
are bound to A rather than B.

### 4.4 Membership generation and reachability

Successful removal advances the existing process-local
`membership_generation` exactly once and removes exactly the selected member.
No new durable generation is introduced.

Rejected malformed, unknown, stale, active, or busy requests do not advance
the generation. In particular, `WorkspaceMembershipState::remove` must not
increment before its existence/inactive check.

After success:

- the removed selector no longer resolves;
- its private repository identity proof is unreachable through membership;
- an old selector cannot be used to activate or execute anything;
- re-admission of the same path performs full current validation and allocates
  a fresh `RepositoryMemberId` in the existing workspace epoch/ordinal scheme;
- the fresh member remains inert until a separate explicit activation; and
- no removed member ID, identity, preparation, ticket, or runtime binding is
  revived.

The membership generation is process-local and nonpersistent. Restart creates
a new empty process-local membership state; the durable remembered catalog is
independent.

### 4.5 Started and uncertain effects

Removal is never an effect-recovery mechanism:

```text
started effect != removed effect
```

If an effect was started for the target, removal rejects while the existing
owner requires it. If the effect becomes uncertain, removal remains rejected
until existing refresh/resolution semantics settle it. Task 340 adds no
cancellation, replay, rollback, compensation, or automatic recovery.

## 5. Prepared-state ownership classification

The following classification is normative for implementation and tests.

| State category | Classification for an inactive member | Task 341 treatment |
| --- | --- | --- |
| Admission publication | MUST NOT EXIST FOR INACTIVE MEMBER | Admission and removal serialize through `membership_coordination`; an in-flight admission publishes only after its own current checks. |
| Activation/switch transition | MUST NOT EXIST FOR INACTIVE MEMBER | Activation publication and removal serialize through `membership_coordination` plus `lifecycle_coordination`; loser returns stale/busy and cannot publish absent member. |
| Repository workflow action/refresh | MUST NOT EXIST FOR INACTIVE MEMBER | Actions are generated for the active repository generation; removal does not clear active workflow state. |
| Stage prepared action | MUST NOT EXIST FOR INACTIVE MEMBER | Switching invalidates it; an observed target-bound inactive action is `busy`, never silently dropped. |
| Stage effect reservation | REMOVAL MUST REJECT WHILE PRESENT if its `member_id` equals target | Existing reservation/currentness owner remains in control; no rollback or replay. An A reservation while removing B is unrelated and preserved. |
| Stage started/uncertain effect | REMOVAL MUST REJECT WHILE PRESENT if target-bound | Existing effect resolution/refresh owns the outcome. |
| Unstage prepared action | MUST NOT EXIST FOR INACTIVE MEMBER | Same as Stage prepared action. |
| Unstage effect reservation | REMOVAL MUST REJECT WHILE PRESENT if target-bound | Same as Stage reservation. |
| Unstage started/uncertain effect | REMOVAL MUST REJECT WHILE PRESENT if target-bound | Same as Stage started/uncertain effect. |
| Completed Stage/Unstage effect | UNRELATED / ACTIVE-ONLY | Existing Git/index result remains; removal does not claim rollback or mutate Git. |
| Commit review prepared | MUST NOT EXIST FOR INACTIVE MEMBER | Switching withdraws review/workflow state; target-bound residue is `busy`. |
| Commit authorization current | MUST NOT EXIST FOR INACTIVE MEMBER | Usable authorization is active-only and currentness-bound; removal of B does not revoke A authorization. |
| HostExplicit prepared ticket | MUST NOT EXIST FOR INACTIVE MEMBER | Switching clears the single prepared coordinator state; target-bound residue is `busy`; an A ticket remains unchanged when B is removed. |
| HostExplicit started/uncertain effect | REMOVAL MUST REJECT WHILE PRESENT if target-bound | Existing coordinator/effect owner resolves it; removal is not rollback. |
| Finished HostExplicit ticket | MUST NOT EXIST AS USABLE STATE | Finished or consumed tickets cannot be revived; status/activity remains sanitized. |
| Conversation in-memory binding | MUST NOT EXIST FOR INACTIVE MEMBER | Current `DesktopConversationState` is active repository-generation-bound; switching starts a new context. Removal does not rewrite active A context. |
| Durable transcript history | UNRELATED / ACTIVE-ONLY AUTHORITY DISTINCTION | History remains namespaced descriptive persistence; removal does not delete, hide, or rewrite it. Fresh admission/activation is required before use. |
| Provider/runtime ownership | MUST NOT EXIST FOR INACTIVE MEMBER | Connection and provider activation are active-only; removing B does not disconnect or rebuild A. |
| Remembered candidate | UNRELATED / ACTIVE-ONLY AUTHORITY DISTINCTION | `remembered-workspace.json` is never touched by removal. |

The matrix deliberately distinguishes “must not exist” structural invariants
from “must reject” effect-bearing conditions. The implementation must not add
deterministic cleanup for state that can carry an external effect.

## 6. Acceptance matrix

The rows below are the required Task 341/342 lifecycle acceptance contract.

| State | Can inactive target own it? | Removal disposition | Cleanup/invalidation | External effect risk | Required deterministic test | Reason / existing owner |
| --- | --- | --- | --- | --- | --- | --- |
| Target member existence | Yes, if admitted and present | Success only if present; otherwise `member_not_found` | Remove only the resolved member | None | Remove existing B; unknown selector; double remove | `WorkspaceMembershipState` owns membership. |
| Target active/inactive | Inactive only | Active target returns `active_member`; inactive may proceed | None on rejection | None | Active A rejection and inactive B success | `active_member` in membership is the authority boundary. |
| Admission transition | No | Serialize; admitted target can be removed after admission linearizes | No admission cleanup | None | Remove vs fresh admit ordering | `membership_coordination` and admission relation checks. |
| Activation transition | No | Exactly one transition wins; removal or activation returns first | Losing activation becomes stale/busy | Publication race | Remove vs B activation; remove vs A→B switch | Activation final gate uses membership and repository generations. |
| Repository workflow action | No | Preserve unrelated active A; target residue is `busy` | No cleanup of active workflow | No new effect | A workflow action unchanged after B removal | `RepositoryWorkflowState` is active-generation-bound. |
| Stage prepared | No | Reject only if target-bound residue is observed; otherwise preserve A state | Existing switch invalidation only | Low before start | A prepared state unchanged; impossible B ownership assertion | Stage actions bind repository/observation generation. |
| Stage effect started | No under invariant | Target-bound removal rejects | Existing effect owner resolves | High | Synthetic target-bound reservation rejects; A effect survives B removal | `RepositoryIndexEffectReservation` owns started lifecycle. |
| Stage uncertain effect | No under invariant | Target-bound removal rejects until refresh/resolution | No rollback/replay | High | Uncertain owner blocks removal; no second attempt | Existing uncertain-effect refresh semantics. |
| Unstage prepared | No | Same as Stage prepared | Existing switch invalidation only | Low before start | A prepared state unchanged; B impossible assertion | Same active-only workflow owner. |
| Unstage effect started | No under invariant | Target-bound removal rejects | Existing effect owner resolves | High | A Unstage effect survives B removal; target-bound fixture rejects | Same index reservation owner. |
| Unstage uncertain effect | No under invariant | Target-bound removal rejects | No rollback/replay | High | Uncertain Unstage owner blocks removal | Same uncertain-effect rule. |
| Commit review prepared | No | Preserve A; target residue is `busy` | Existing switch invalidation only | None before authorization | A review unchanged; no B review can execute | `RepositoryWorkflowState` and Commit review currentness. |
| Commit authorization current | No | Preserve A; target residue is `busy` | No invalidation of unrelated A authority | High if wrongly migrated | A authorization currentness unchanged after B removal | `DesktopCommitCapability` is singular, active-only, generation-bound. |
| HostExplicit prepared ticket | No | Preserve A ticket; target residue is `busy` | No ticket clear for B removal | Medium/high | A ticket remains ticket-valid; stale B ticket cannot confirm | `HostInvocationCoordinator` is singular and current-composition-bound. |
| HostExplicit started tool effect | No under invariant | Target-bound removal rejects; A effect unaffected | No cancellation/replay | High | Started target owner rejects; A started state unchanged | Existing Started/uncertain HostExplicit lifecycle. |
| Conversation bound to target | No executable binding | No removal cleanup; target-bound residue is stale/busy | In-memory context is active-only | Model-context risk, not repository effect | A conversation identity/history unchanged | `DesktopConversationState` binds current repository generation. |
| Provider/runtime bound to target | No | Target residue is `busy`/internal contradiction; do not tear down | No provider teardown | High | A runtime/provider Arc identity unchanged | `ConnectionState` and `provider_activation` are active-only. |
| Remembered candidate exists | Yes, independently | Remove member successfully | Catalog untouched | None | Candidate remains after B removal | ADR 0028 store is a separate owner. |
| Remembered candidate absent | N/A | Removal semantics unchanged | No catalog write | None | Delete candidate before removal; remove still succeeds | No cross-coupling between catalog and membership. |
| Restart | No process-local member survives | Startup has zero executable membership | Durable catalog may load inertly | None | Restart after removal yields zero members/authority | ADR 0027/0028 restart contract. |
| Stale selector after removal | No | `member_not_found` with zero effect | No generation change | None | Repeat old B selector | Fresh membership lookup fails closed. |
| Fresh re-admission of same path | New member only | New admission succeeds subject to current validation | New ID; no old ID revival | None | New ID differs from old B and stays inactive | `admit` allocates fresh ordinal; activation remains separate. |

## 7. Conversation, provider, Commit, Stage/Unstage, and remembered isolation

### 7.1 Conversation

The current in-memory conversation state is keyed by active repository
generation and model generation, not retained as an executable context for an
inactive member. Activation/switch starts a new in-memory context. Durable
SQLite transcript history is descriptive, namespaced by repository context,
and separate from current repository authority.

Therefore removal of inactive B:

- does not delete, hide, or rewrite B's durable transcript history;
- does not rewrite conversation repository IDs or namespaces;
- does not change A's active conversation identity or history;
- makes any old B executable context unreachable because B's member selector is
  removed and fresh admission/activation is required; and
- cannot make stale transcript content authorize a Tool, repository, provider,
  or runtime action.

### 7.2 Provider/runtime

The source confirms active-only ownership. `ConnectionState::Connected` binds
one runtime to one repository generation and one published composition;
`provider_activation` is the corresponding current provider composition.
Inactive membership does not own either object. Removing B must not acquire
provider/runtime locks for teardown and must not disconnect or rebuild A.

If a future implementation introduces inactive live ownership, that is a
contradiction requiring a new architecture decision and is a Task 340 STOP,
not a Task 341 cleanup shortcut.

### 7.3 Commit

There is at most one executable reviewed Commit capability for the active
repository. Its review, authorization, repository generation, model
generation, identity generation, and currentness are active-bound. Switching
withdraws it. Removing inactive B does not invalidate A's current review or
authorization and does not grant any authority to B.

If target-bound Commit state is ever observable for an inactive member,
removal returns `busy`; it does not clear or migrate that state.

### 7.4 Stage and Unstage

Stage and Unstage remain separate host index authorities. Their action IDs and
reservations bind active member/repository identity, repository generation,
observation generation, target observation, and action kind. Switching
invalidates old actions; an A action cannot execute against B.

Removal of inactive B preserves A's Stage/Unstage presentation and any
unrelated active reservation. A target-bound B reservation or started/uncertain
effect is an architecture violation or in-flight owner and returns `busy`.
No path in Task 340 claims to undo a completed index effect.

### 7.5 HostExplicit

HostExplicit remains exactly the existing 11 tools. Tickets are opaque,
single-use, process-local, current-composition-bound, and nonpersistent. There
is no per-inactive-member queue. Switching clears the prepared coordinator
state and a started effect cannot be replayed.

Removal of B preserves an A-bound prepared ticket/effect and rejects any
observable B-bound prepared/started/uncertain owner. A removed selector cannot
make a stale ticket current again. No ticket type or eligibility set is added.

### 7.6 Remembered workspace

Removal never calls the remembered workspace mutation owner and never writes
`remembered-workspace.json`. The following are all independent and valid:

- a remembered B candidate exists before member removal and remains afterward;
- the candidate is deleted before member removal and removal has the same
  member semantics;
- the candidate is deleted after member removal and removal remains complete;
- later explicit admission from the remembered candidate creates a new member
  ID; and
- later explicit admission does not automatically activate the fresh member.

## 8. Lock ordering and linearization

### 8.1 Required order

Removal must follow this synchronous order, with no `.await` while the
coordination locks are held:

```text
parse selector outside locks
  -> acquire membership_coordination
  -> acquire lifecycle_coordination
  -> inspect/revalidate membership and target ownership
  -> acquire workspace_membership
  -> prove target exists and is inactive
  -> prove no target-bound owner can publish/complete an effect
  -> call WorkspaceMembershipState::remove exactly once
  -> release workspace_membership
  -> release lifecycle_coordination
  -> release membership_coordination
  -> build and return sanitized membership presentation
```

The implementation may take individual state locks in the established
active-lifecycle order after the two coordination locks. It must not acquire a
provider/model/runtime lock while mutating membership unless a current owner
check demonstrably requires it. In particular, no provider shutdown or
conversation persistence operation runs inside this transaction.

The order is compatible with existing activation publication:

```text
membership_coordination
  -> lifecycle_coordination
    -> workspace_membership / repository / repository_generation /
       chat / connection / host_invocation / workflow / Commit state
```

Admission already serializes under `membership_coordination`; it must not be
changed to acquire lifecycle state for this feature. Existing lifecycle
mutations acquire `lifecycle_coordination` and never acquire
`membership_coordination` after it. Any new Task 341 helper must preserve this
order and must not create a reverse `lifecycle_coordination ->
membership_coordination` path.

### 8.2 Currentness checks

The remove transaction rechecks the member after acquiring both coordination
locks. It does not rely on a pre-lock presentation snapshot. A successful
removal is the only point at which the member map changes.

Activation already captures member identity/admission generation, expected
active member, and expected repository generation, and rechecks all of them at
the publication linearization point. Removal uses the same owner boundary:

- if removal wins first, activation's final member lookup fails stale/not-found;
- if activation wins first, B becomes active and removal returns
  `active_member`; and
- no activation can publish a repository after B has been removed.

Removal does not increment `repository_generation`, because it does not change
the active repository. It does not alter model, profile, connection, or
composition generations. It advances only `membership_generation` after the
member-map deletion succeeds.

### 8.3 Race outcomes

| Race | Deterministic outcome |
| --- | --- |
| Remove B vs activate B | Shared coordination serializes the linearization. Removal first makes activation stale/not-found; activation first makes removal reject active. |
| Remove B vs switch A → B | Same serialization. There is no partial switch and no active repository pointing to an absent member. |
| Remove B vs re-admit same repository | Remove first permits fresh admission; admission first is subject to existing duplicate/alias rules. A fresh admission never revives old B ID. |
| Double remove B | First request succeeds; second returns `member_not_found`/stale and does not advance generation. |
| Remove vs remembered-candidate delete | Independent owners; either order is valid and has no cross-coupled result. |
| Remove B vs active A chat/connection/HostExplicit/index effect | B removal preserves A state. It does not use active-selection busy checks that would withdraw unrelated A state. |

The guarantee is serialization/currentness within the process. It is not a
claim of race-free filesystem TOCTOU or cross-process coordination.

## 9. UI/product behavior to freeze for later tasks

For an inactive member, Task 342 displays:

```text
Remove
```

For the active member, the preferred presentation is no Remove action. If a
disabled action is shown, its bounded explanation is:

```text
Switch to another repository before removing this active repository.
```

The confirmation copy must state:

- repository files are not deleted;
- the remembered entry is not forgotten; and
- only current process membership is removed.

The confirmation contains no native path or private identity. The handler
passes only the opaque member selector to the one removal command. It does not
chain activation, remembered-candidate deletion, or any other command.

## 10. Deterministic test ownership

### 10.1 Membership unit tests for Task 341

Task 341 owns tests for:

- inactive removal success;
- active removal rejection with byte-for-byte/equivalent membership snapshot
  and generation unchanged;
- malformed selector;
- unknown selector;
- first removal success and second removal stale/not-found;
- generation increment exactly once on success and never on rejection;
- old selector invalidation; and
- same-path re-admission receiving a fresh member ID that remains inactive.

The unit tests must specifically catch the current `remove` ordering defect:
an absent or active call must not advance `membership_generation`.

### 10.2 Desktop lifecycle tests

Task 341 backend tests and Task 342 integration tests cover:

- A active/B inactive removal preserving A `DesktopRepository`, repository
  generation, active ToolRegistry/composition identity, provider/runtime,
  conversation identity/history, Commit state, and Stage/Unstage state;
- active A rejection with zero repository, Git, provider, runtime,
  conversation, and membership effect;
- unknown and stale selector zero effect;
- A review/authorization isolation;
- A Stage and Unstage isolation, including an active reservation/effect;
- active provider/runtime isolation;
- conversation and durable transcript isolation;
- remembered-candidate isolation for present and absent candidates;
- remove/activate and remove/switch serialization using existing test barriers;
- remove/re-admit ordering and fresh-ID proof; and
- target-bound started/uncertain owner rejection without rollback, replay, or
  cancellation.

### 10.3 Privacy tests

Serialize every result, error, membership DTO, optional Activity event, and
test-visible status. Assert that none contains sentinel native paths, Git
directories, filesystem IDs, remembered location hints, authority handles,
tickets, provider handles, or raw errors. The test must inspect actual values,
not only field names.

### 10.4 Later Tauri/frontend tests

Task 342 owns tests that:

- show Remove only for inactive members;
- disable or reject active removal with bounded copy;
- confirm membership removal versus disk/catalog deletion;
- invoke only the closed removal command;
- do not chain activation or remembered-candidate mutation; and
- refresh membership after success and remove the stale selector from the UI.

## 11. Required acceptance scenarios

Task 341/342 must make these exact postconditions executable:

| ID | Scenario | Required result |
| --- | --- | --- |
| A | Remove inactive B while A active | Success; B absent; A active; active composition/runtime/provider/conversation/Commit/Stage/Unstage unchanged; Git state unchanged. |
| B | Remove active A | `active_member`; zero effect. |
| C | Unknown selector | `member_not_found` or selector-invalid category as applicable; zero effect. |
| D | Stale B selector after success | Bounded not-found/stale; zero effect and no generation change. |
| E | Double removal | First success; second bounded failure. |
| F | Re-admit B path | New member ID differs from old B; fresh member remains inactive. |
| G | Remembered B exists | Removal succeeds; remembered candidate remains unchanged. |
| H | Remembered B deleted first | Removal semantics unchanged; no catalog dependency. |
| I | Active A has Commit state | Removing B does not alter A review/authorization/currentness. |
| J | Active A has Stage/Unstage state | Removing B does not alter A index-authority state or claim rollback. |
| K | Active A is connected | Removing B does not disconnect or rebuild the runtime/provider. |
| L | Removal races B activation/switch | Shared serialization/currentness yields one winner; never active absent member. |
| M | Target-bound started/uncertain effect | Reject while owner is present; no rollback/replay/cancellation. |
| N | Restart after removal | Process-local membership and executable authority are zero; remembered candidates retain independent semantics. |

## 12. Task 343 live-certification contract

Task 343 must use the exact clean candidate SHA and fresh disposable Windows
repositories A and B. The evidence is host-driven production-backend evidence,
not GUI evidence.

| Step | Required evidence |
| --- | --- |
| 1 | Admit A and B through the production host path; record only bounded member selectors. |
| 2 | Activate A; capture A selector, B selector, active repository presentation, ToolRegistry/composition identity, runtime/provider state, conversation binding, Commit state, and Stage/Unstage state. |
| 3 | Remove inactive B; prove B absent, B selector stale, A still active, active composition preserved, runtime/provider preserved, conversation preserved, and active Commit/Stage/Unstage state unchanged. |
| 4 | Attempt active A removal; prove bounded active rejection and zero effect. |
| 5 | Re-admit B from its path or remembered candidate; prove fresh member ID differs from old B and remains inactive. |
| 6 | Verify remembered catalog bytes/semantic presentation are unchanged by member removal. |
| 7 | Verify A and B Git integrity, including HEAD/refs/index/worktree evidence relevant to the fixture. |
| 8 | Restart; prove zero executable membership, active repository, repository authority, ToolRegistry, provider/runtime, Commit, Stage/Unstage, HostExplicit, and executable conversation binding. |
| 9 | Record zero model requests, model Tool requests, MCP activations, Process Plugin activations, network Git, automatic commits, and automatic activations. |
| 10 | Clean only attributable processes and exact disposable roots; do not delete historical matching roots or claim cleanup beyond the current fixture. |

Required live markers should identify removal success, stale-selector proof,
active-preservation proof, fresh-ID re-admission, remembered-catalog
preservation, and restart zero-authority proof without exposing paths or
private identity evidence.

The certification must explicitly disclaim:

- GUI automation unless it is actually run;
- model-selected dynamic Tool dispatch;
- cross-platform live parity;
- rollback, replay, compensation, or recovery guarantees;
- race-free TOCTOU;
- OS sandboxing or network isolation; and
- cross-process or linked-worktree semantics.

## 13. Scope, dependencies, and ADR disposition

### In scope for v0.28

- one explicit host removal of an existing inactive process-local member;
- stale-selector invalidation and fresh re-admission identity;
- active-member rejection;
- lifecycle-busy rejection where an existing owner requires it;
- Desktop host command and sanitized presentation;
- deterministic backend/lifecycle/race/privacy tests; and
- Windows host-driven certification.

### Explicitly out of scope

Active-member removal, switch-and-remove, bulk/remove-all, durable membership
or removal, remembered-candidate deletion, filesystem/Git/worktree deletion,
linked worktrees, provider/model-driven removal, a generic lifecycle manager,
new HostExplicit tools or eligibility, new repository mutation authority,
persistence schema/migration, browser persistence, model Tool exposure, and
automatic recovery.

Task 340 changes no Cargo manifest, lockfile, dependency, permission, ADR,
frontend, Tauri command, production Rust, test Rust, HostExplicit set, or
Codex baseline. Validation for this document is:

```text
git diff --check
cargo metadata --no-deps --format-version 1
```

The metadata result must continue to report 13 packages, all `0.27.0`, Rust
edition 2024, and no Cargo.lock or dependency change.

No new ADR is required. ADR 0027 already covers the inactive-only
process-local removal boundary. ADR 0028 remains unchanged and continues to
isolate durable remembered candidates from executable membership. If source
review ever proves that an inactive member can retain a live executable
ToolRegistry or provider/runtime, that safe removal requires active authority
withdrawal, a new authority category, durable schema, an unsafe selector
ambiguity, or a race not resolved by the existing coordination/currentness
owners, work stops and returns to ADR research.

## Task 341 — Desktop Inactive Repository Member Removal Foundation

Task 341 is production implementation focused on:

- the backend host removal function;
- one closed Tauri command accepting only the existing bounded member selector;
- reuse of `WorkspaceMembershipState::remove` as the sole membership mutation
  primitive, with its rejected-attempt generation ordering corrected;
- lifecycle/currentness checks using the existing coordination owners and
  active-only structural invariants;
- a sanitized `removed` result and bounded failure mapping; and
- deterministic backend tests for the membership, isolation, privacy, and
  serialization contracts above.

Task 341 must not add full Desktop UX, active-member removal, persistence,
automatic switching, model/Tool exposure, new authority, new HostExplicit
eligibility, provider teardown, transcript deletion, or Cargo/dependency
changes.
