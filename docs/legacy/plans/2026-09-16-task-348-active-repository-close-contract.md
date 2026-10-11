# Task 348 — Active Repository Close Contract and Lifecycle Acceptance Matrix

Status: Implementation-ready contract for Task 349; no implementation in Task 348.

Baseline: `04bc19a3ed7f662b9955880481e68a7897159b9d` (Task 347 exact-head CI `35099997493` PASS).

Release/workspace baseline: published RAH `v0.28.0`; 13 workspace packages,
all `0.28.0`, Rust edition `2024`; certified Codex baseline `0.149.0`.
HostExplicit remains exactly these 11 tools:
`fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`,
`repo.create-branch`, `repo.patch`, `repo.edit-files`, `repo.create-file`,
`repo.delete-file`, and `repo.rename-file`.

## Decision and scope

V0.29 adds one explicit human Desktop action: Close the active repository. Close
is host-owned process-local authority withdrawal, classification **NONE**. It
does not add an authority, Tool, permission, HostExplicit eligibility, provider
action, persistence format, or architecture boundary. No new ADR is required:
ADR 0027 expressly accepts zero active repositories, inert membership, and
fresh active-only composition; ADR 0028 remains unchanged and keeps remembered
workspace data descriptive. HostExplicit remains exactly 11.

The transition is:

```text
members = [A, B?], active = A
  -- explicit, intent-bound Close A -->
members = [A, B?], active = none, DesktopRepository = none
```

A remains admitted with the same process-local `RepositoryMemberId`, admission
identity record, admission generation, and presentation label. B, if present,
remains admitted and inactive. Close neither changes nor reorders membership,
admits or activates a member, nor changes remembered-candidate presentation.
The action is not Disconnect, Switch, Remove, Forget, repository deletion, Git
mutation, Tool invocation, provider/profile reload, or a combined transaction.

## Production ownership findings

The current Windows Desktop production implementation was traced through
`crates/rah-desktop/src/repository_membership.rs`,
`crates/rah-desktop/src/main.rs`, `crates/rah-desktop/src/host_invocation.rs`,
`crates/rah-desktop/src/effective_authority.rs`, and
`crates/rah-desktop/src/conversation_persistence.rs`.

| Owner | Current production representation and consequence for Close |
| --- | --- |
| Membership | `WorkspaceMembershipState` owns a map of `InertRepositoryMember` and one `active_member`. Members retain private root/identity/admission data only; no member retains a ToolRegistry, provider, runtime, Commit, index, or HostExplicit queue. `admit`/`remove` change `membership_generation`; `publish_active` changes only the active member. |
| Active repository | `DesktopAppState.repository: Option<Arc<DesktopRepository>>` is the current executable repository object. `publish_active_repository` publishes it with membership and increments `repository_generation`. Activation revalidates the retained admission identity, reconstructs `DesktopRepository` and its authorities, and publishes a new repository generation. |
| Registry/composition | The connected `ConnectionState::Connected` owns the runtime and `DesktopToolComposition`; `provider_activation` separately owns the active external provider composition. The repository member map owns neither. With `NotConnected` and `provider_activation == None`, no connected repository ToolRegistry/composition or runtime is retained. Trusted Profile selection is separate global host configuration and remains selected/inert. |
| Active/repository currentness | There is no separate active-selection-generation field. `repository_generation` is the existing active/repository currentness used by repository snapshots/actions, Commit captures, conversation identity, connection publication, and HostExplicit generation checks. It increments on each activation/switch publication. `membership_generation` is separate and changes only when membership changes. |
| Repository workflow | `RepositoryWorkflowState` owns observation/action generations, Stage/Unstage action selectors, staged review, Commit review and selector, and authorization presentation. It is in-memory and repository-generation-bound. Resetting it to `default()` invalidates no-effect selectors/reviews without touching Git. |
| Stage/Unstage effect | `repository_index_effect_reservation` owns the one active index lifecycle reservation and captured `DesktopRepository`. A reservation exists before native invocation and remains through existing post-effect refresh. It is the real fail-closed busy owner; Close must never remove it. |
| Commit | `commit_capability` owns the active control/tool and generation binding; its `RepositoryCommitControl` owns the one-shot pending authorization. `repo.commit` is not HostExplicit and current Desktop execution is inside a model turn. Workflow owns the reviewed snapshot/selector. Disconnect already revokes the control/workflow. Close clears any safe pending authorization and capability; a running Commit is covered by the active model-turn owner and is busy. |
| HostExplicit | One global `HostInvocationCoordinator` is `Idle`, `ModelTurn`, `HostPrepared`, or `HostRunning`; `HostPrepared` includes asynchronous zero-effect preparation, and `HostRunning` covers dispatch through terminal handling. Tickets and preparations are in-memory only. `repo.commit` is ineligible; the exact eligible set remains the existing 11. |
| Chat/model turn | `chat`, `active_chat`, and `HostInvocationCoordinator::ModelTurn` jointly own a turn and its captured context. Start, terminal completion, and cancellation publication coordinate through `lifecycle_coordination`. A turn must be idle/absent before Close. |
| Connection/provider | `ConnectionState` includes `NotConnected`, `Connecting`, `Connected`, `Disconnecting`, and `Error`; `provider_activation` owns effective external providers. Pending connection construction remains represented as `Connecting` until publication/rejection. Close does not shut down or reap either owner. |
| Conversation | `DesktopConversationState` owns in-memory `identity`, model `history`, and an in-memory epoch. `Persistence` owns completed transcript bytes in independent repository/neutral namespaces. `start_new()` clears only in-memory binding/history and increments its epoch; no persistence write is needed for Close. |
| Remembered candidates | `RememberedWorkspaceState` is separate. Membership activation currently does not update `last_active_member_id`; Close must not call any catalog mutation or write `remembered-workspace.json`. |

One existing stale-publication window must be closed in Task 349: an async
`refresh_repository_workflow` captures A and a repository generation, checks
that generation after Git observation, then installs workflow state without
the lifecycle gate. Close can otherwise clear the workflow between the check
and install. The Task 349 final workflow check-and-install must take
`lifecycle_coordination` synchronously and recheck the captured repository
generation/current repository before publishing; stale refresh returns the
existing bounded observation-stale/failure outcome and publishes nothing.
This is a narrow currentness completion, not a new owner or authority.

### Generation/currentness decision

| Value | Close behavior | Reason |
| --- | --- | --- |
| `membership_generation` | Unchanged. | Close retains exactly the same members and ordering. It must not disguise authority withdrawal as membership mutation. |
| `RepositoryMemberId`, workspace epoch, member admission generation, identity record | Unchanged and retained for A. | A remains admitted; these values identify the process-local admission, not active authority. |
| `repository_generation` | Advance exactly once at successful Close, using a checked increment precomputed before any mutation; activation/switch publication also uses checked arithmetic. | This is the existing currentness axis that invalidates active selection, repository actions, Commit captures, connection/runtime publication, and conversation context. Exhaustion returns bounded `unavailable` with zero state change; do not wrap and revive stale values. |
| Separate active-selection generation | Do not add. | No separate field exists, and the existing repository generation safely expresses each activation/withdrawal. |
| Workflow observation/action generations and selectors | Clear with the workflow state. Do not preserve/reuse selectors. | Repository generation changes; later activation and observation create fresh selectors/currentness. |
| Commit capability/control and pending authorization | Remove the capability and clear the pending authorization as part of the no-effect publication; a nonblocking-clear failure rejects before publication. | Existing Commit authority remains the authority model; no new Commit category or generation is introduced. |
| Model, profile, connection, Commit-identity generations | Unchanged by Close. | Close neither changes model/profile configuration nor connects/disconnects runtime. A stale connection publication is guarded by repository generation and connection state. |
| HostInvocation ticket/invocation counters | Unchanged. | Clearing a prepared ticket does not mint authority or require counter churn. `HostRunning` rejects Close. |
| Conversation epoch | `start_new()` advances its existing in-memory epoch once. | This rejects any late in-memory completion binding; Close itself does not persist a separator or transcript change. |
| Registry/composition identity | No active repository-bound identity survives Close. | There is no inactive-member registry. Explicit later Connect creates a fresh connected composition over current active state. |

Every prior A repository-generation-bound snapshot, connection publication,
HostExplicit ticket, Commit review/authorization, Stage/Unstage selector, and
conversation completion becomes stale or absent. Explicit later activation
advances repository generation again and rebuilds `DesktopRepository` and its
authorities from a freshly revalidated retained admission record. A later
explicit Connect, if requested, composes a new active-only ToolRegistry and
provider/runtime state. No old value becomes current by reopening A.

Task 349 must use the checked repository-generation publication helper for
activation/switch as well as Close, replacing the current `wrapping_add` at
active publication. Otherwise a later activation could wrap the same currentness
axis and eventually revive a stale external selector. Exhaustion fails before
any authority revocation/publication; this is the existing in-memory generation,
not a new/durable epoch.

## Public intent and input contract

The UI must bind confirmation to the exact member and active snapshot the human
is looking at. A no-argument “close whatever is active now” command is rejected
as a design: a delayed UI confirmation could otherwise close B after A→B.

The closed request is exactly:

```text
CloseRepositoryRequest (Rust fields) = {
  expected_active_member_id: RepositoryMemberId selector,
  expected_repository_generation: positive u64
}
JSON IPC fields are exactly `expectedActiveMemberId` and
`expectedRepositoryGeneration`.
```

The frontend captures the active member selector from the membership
presentation and `repository.currentGeneration` from the displayed
Effective Authority snapshot before opening confirmation. It sends those
captured values only after the human confirms. The two reads may be from
different render moments; that can produce a safe stale rejection, never a
different repository close. Task 350 must preserve the captured values while
the confirmation dialog is open rather than rereading “current active” on
Confirm.

The selector and generation are equality/currentness guards only. They are
not authority. Under `membership_coordination` and
`lifecycle_coordination`, the backend resolves the actual member, verifies it
is still active, verifies the active `DesktopRepository` exists and is the
host-owned repository for that member, and compares the captured generation
with current host state. The backend derives all repository binding and
authority from current host state. A malformed/unknown selector or malformed
guard is rejected. A well-formed old guard yields `active_changed` if another
member is active. It never closes that member.

The public boundary rejects unknown fields and accepts no path, canonical
root, `.git` path, filesystem identity, Git executable identity, remembered
candidate ID, Tool/provider/permission value, registry handle, ticket, or
source-bearing value. There is no path, label, remembered-candidate, or
current-active fallback. The member selector and generation cannot grant
authority or be used by any model/Tool route.

If there is no active member, a structurally coherent zero-active state returns
`no_active` with zero mutation and no generation churn, regardless of an old
well-formed guard. If membership says no active member but repository-bound
executable state remains, fail closed as `unavailable`; do not silently repair
the inconsistent state or claim Close succeeded.

## Preconditions and effect ownership

### Runtime/provider precondition

Success requires exactly:

```text
ConnectionState::NotConnected
provider_activation = None
no connection/provider publication in progress
```

`Connecting`, `Connected`, `Disconnecting`, and `Error` are rejected as
`connected_or_runtime_busy`. `Connecting` covers local pending runtime and
provider composition as well as publication; `Disconnecting` covers the
existing shutdown lifecycle; `Error` is not proof that runtime/provider
ownership has been cleanly withdrawn. `provider_activation != None` with
`NotConnected` is an invariant mismatch and fails closed. Close never stops
Codex, MCP, Process Plugin, or another runtime; waits for shutdown; destroys a
Trusted Profile; retries Disconnect; or infers shutdown from an error.

The human must complete the existing explicit Disconnect lifecycle first. If a
HostExplicit ticket is prepared, the current Disconnect command refuses while
the coordinator is `HostPrepared`; the ordinary user sequence is to explicitly
Cancel that no-effect ticket, then Disconnect, then Close. A `HostPrepared`
state encountered under the accepted disconnected precondition is therefore
normally structurally absent, but Close still defensively invalidates it (and
any async prepare can no longer finalize) because it has no started effect.
The deterministic test may seed this no-effect state directly; it must not
claim that a public disconnected flow normally retains such a ticket.

The current Disconnect command rejects `ModelTurn` and `HostPrepared` but does
not reject `HostRunning`. Consequently `NotConnected` alone is not a sufficient
Close precondition: a started HostExplicit operation can remain owned after
runtime/provider withdrawal. The separate HostRunning check must return
`repository_effect_busy` and preserve that owner until its existing terminal
handling finishes.

Similarly, current Disconnect already removes Commit capability/workflow and
pending Commit authorization before it publishes `NotConnected`. Close still
clears these safe no-effect owners defensively. Stage/Unstage action selectors
and repository observations can remain after Disconnect and are ordinary
Close invalidation targets.

### Model-turn precondition

Close returns `model_turn_busy` if `HostInvocationCoordinator` is
`ModelTurn`, `chat != Idle`, or `active_chat` owns a turn. These checks are
redundant evidence for one real lifecycle owner, not a generic busy flag. Close
does not cancel generation, rebind a turn, migrate it, discard a Tool result,
or choose a new transcript context. A started-under-A turn must finish or be
handled under the existing explicit cancellation/terminal-ownership protocol
before Close can succeed.

### Owner-by-owner policy

| Owner/state | Close disposition | Required action and why |
| --- | --- | --- |
| Repository workflow observation/review state | `INVALIDATE ON CLOSE` when no effect owner is active. | Reset observation, review, Commit snapshot/selector, and status state; suppress a late refresh publication by final lifecycle-gated currentness check. Observation is descriptive/read-only. |
| Stage/Unstage displayed selector or prepared action | `INVALIDATE ON CLOSE`. | Clear selector/action state. It has not reserved or started a native index effect; later activation requires fresh observation and selection. |
| Stage/Unstage reservation, native operation in progress, or unresolved post-effect owner | `CLOSE MUST REJECT AS BUSY`. | Any `repository_index_effect_reservation` is retained. Reservation begins before native invocation and protects through existing completion/refresh, so it is conservatively busy even in its pre-execution reservation interval. Never clear it or infer an effect outcome. |
| Reviewed Commit snapshot/review not yet authorized | `INVALIDATE ON CLOSE`. | Drop workflow review/selector. Later activation needs new observation/review. |
| Pending Commit authorization not yet started | `INVALIDATE ON CLOSE`, after a successful nonblocking `try_clear_authorization_now`. | Drop the current Commit capability and workflow state. If the control cannot be synchronously cleared, return `repository_effect_busy` before publication; do not await while holding coordination or publish partial withdrawal. |
| Commit execution / started or unresolved Commit effect | `CLOSE MUST REJECT AS BUSY`. | Current Desktop `repo.commit` execution is under the captured model turn; Close is blocked by that owner. There is no separate Desktop Commit execution reservation. Do not clear a running control, claim success/no effect, or replay. |
| HostExplicit prepared ticket or async preparation | `INVALIDATE ON CLOSE` if present and no started effect exists. | `HostInvocationCoordinator::clear_prepared()` consumes only `HostPrepared`, including `preparing`; a later finalize/Confirm fails because its coordinator state/ticket is gone. The state is normally absent after required Disconnect, as described above. |
| HostExplicit `HostRunning` / Started / unresolved or lost result | `CLOSE MUST REJECT AS BUSY`. | Preserve coordinator owner until its existing terminal handling completes. The coordinator does not distinguish read versus effectful running Tool, so conservatively reject every `HostRunning`; do not infer no effect from timeout, cancellation, disconnect, crash, or lost response. Once existing owner reaches its terminal boundary, Close does not replay or compensate it. |
| Active model turn | `CLOSE MUST REJECT AS BUSY`. | Preserve `ModelTurn`/chat/terminal ownership and captured A context. |
| Provider/runtime activation/publication | `MUST ALREADY BE ABSENT`. | Proven by exact `NotConnected` plus `provider_activation == None`; Connecting/Disconnecting/Error/Connected rejects. |
| Activation/switch transaction in pre-publication validation | `CLOSE` and final activation publication serialize under membership→lifecycle gates. | If Close wins, repository-generation/expected-active guard makes the captured activation stale. If activation publishes first, Close's guard is stale. No partially built target is retained by membership. |
| Remembered workspace candidate/catalog | `UNRELATED`. | Preserve every catalog byte and descriptive field. Close is not Remember, Forget, reorder, or last-active update. |
| Completed transcript persistence | `UNRELATED`. | Preserve bytes and namespace contents. Close clears only in-memory `DesktopConversationState`; it neither deletes, migrates, rewrites, appends a separator, nor automatically resumes durable history. |
| Global Trusted Profile selection/configuration | `UNRELATED`. | Keep it selected as inert host-owned global configuration. It cannot create repository authority with no active repository and disconnected runtime. |
| A's admitted private identity/root record and user-facing member label | `UNRELATED` / retained. | Membership remains exactly the same. Do not destroy A to clear `DesktopRepository`. |
| B's inactive membership/presentation | `UNRELATED`. | B remains independently admitted and inactive; no focus, activation, registry, or union composition. |

No generic `busy` flag or uncertain-effect ledger is introduced. Busy comes
from the existing named owners: connection/provider state, chat/coordinator,
index reservation, and Commit-control nonblocking exclusion. No-effect
state is invalidated; started/unresolved ownership is left intact and blocks
Close. Terminal uncertainty remains governed by its existing owner and
no-replay lifecycle; Close is possible only after that owner has reached its
existing terminal boundary, and Close never interprets that as proof that the
external effect did not happen.

## Lock order and linearization

### Established coordination order

Close reuses exactly the existing lifecycle serialization; it adds no global
Close mutex:

```text
membership_coordination
  -> lifecycle_coordination
    -> short, synchronous host-state reads / no-effect invalidation
```

Production paths reviewed:

| Path | Existing coordination order |
| --- | --- |
| Admission | `membership_coordination` only, then membership state. |
| Activation/switch capture and publication | `membership_coordination` → `lifecycle_coordination` → bounded state locks. Filesystem/Git identity validation occurs outside the final publication gates; publication rechecks captured member/admission/active/repository currentness. |
| Inactive-member removal | `membership_coordination` → `lifecycle_coordination` → membership and busy-owner state. |
| Connect, provider publication, Disconnect, chat start/terminal, HostExplicit prepare/confirm/cancel, index reservation, Commit-review authorization capture/final publication | `lifecycle_coordination` → relevant short state locks. Async work releases it and must revalidate at publication. |

No reviewed production path acquires `membership_coordination` while already
holding `lifecycle_coordination`; lifecycle-only paths never acquire the
membership coordination mutex. Admission's membership-only order cannot form
a cycle. State mutexes are taken/read and dropped one at a time under the two
gates rather than adding a new nested leaf-lock order. Do not hold a synchronous
guard across `.await`, Git, filesystem identity probing, provider operation,
event emission, or other fallible external work.

### Close algorithm and publication point

The implementation must follow this synchronous protocol:

1. Parse the closed request before taking locks. Reject malformed/unknown
   fields with `invalid_guard` and zero effect.
2. Acquire `membership_coordination`, then `lifecycle_coordination`.
3. Under those gates, read membership active ID/member, `DesktopRepository`,
   `repository_generation`, connection/provider state, chat/active-chat,
   HostInvocation state, index reservation, Commit capability/control, and
   workflow state. Check exact expected member+generation and host-owned
   active/repository coherence. Do not derive authority from request values.
4. Return `no_active` if and only if active member is none and the no-active
   executable state is already coherent. Return `active_changed` for a
   different active member/currentness. Apply busy outcomes in this deterministic
   order: `model_turn_busy`, then `repository_effect_busy`, then
   `connected_or_runtime_busy`. This preserves the active owner reason when,
   for example, a HostRunning call outlives provider Disconnect. Precompute a
   checked next repository generation.
5. The last fallible operation is the nonblocking Commit-control clear. If it
   cannot be obtained, return `repository_effect_busy` with all prior state
   intact. No native or provider effect is attempted.
6. With no remaining fallible operation, clear a no-effect HostPrepared ticket,
   remove Commit capability, replace repository workflow with default, clear
   `DesktopRepository`, install the precomputed next repository generation,
   and `start_new()` the in-memory conversation. Do not touch persistence or
   the remembered catalog. Do not clear an index reservation or any running
   owner.
7. As the final state publication under both gates, set
   `WorkspaceMembershipState.active_member = None` while retaining every
   member. This final active-member publication is the conceptual Close
   linearization point. Before it, all authority/currentness-acquiring paths
   remain excluded and A's old publication is not partially consumable. At
   it, the repository is already absent, generation is fresh, no-effect
   repository state has been invalidated, and A remains a member but is
   inactive. After it, every authority/currentness-acquiring path sees the
   coherent zero-active state when it acquires the existing gates.
8. Build the bounded result from retained member IDs and bounded
   display-name/basename presentation while membership is stable; release
   gates, then return it. Emit no Activity event and perform no follow-up
   fallible effect.

The commit block is non-yielding. All failure checks, including state
coherence, generation exhaustion, connection/model/effect checks, and Commit
control availability, happen before the no-fail state assignments. Thus a
failure before publication preserves A's old state. After the last fallible
check there is no await, external call, event callback, or `Result`-returning
operation that could expose a partial withdrawal. All authority/currentness
acquisition paths are serialized by the lifecycle gate; Task 349 must preserve
that property and synchronize workflow publication as identified above.

## Race outcomes

All races use barriers/hooks at real capture and publication boundaries. Tests
must not use sleeps. “No replay” means losing/stale work is rejected without
reissuing a Tool, native effect, provider operation, activation, or user intent.

| Race | Coordination owner | Allowed winner/outcome | Stale/currentness loser | Zero-effect and no-replay rule |
| --- | --- | --- | --- | --- |
| Close vs Activate A | Both final publications use membership→lifecycle. | Close first publishes none; an activation captured before it fails. A separately initiated activation captured after it may succeed. An already-active no-op that completes before Close changes nothing; Close may then succeed. | Pre-Close activation must match captured active member and repository generation at publication; otherwise stale. | No stale activation after Close; no auto-reactivation. |
| Close vs Switch A→B | Same membership→lifecycle publication gates. | Switch first publishes B/fresh generation; Close-for-A returns `active_changed`. Close first publishes none; captured switch fails stale. | Never reinterpret Close A as Close B; never reinterpret an old switch as a new human action. | No partial composition/union and no replay of switch. |
| Close vs Admit B | Both serialize on membership coordination; admission needs no lifecycle lock. | Admission first: B exists inactive when A closes. Close first: B admission follows and remains inactive. | None; admission does not select. | Member count changes only for explicit admission; Close itself does not. |
| Close vs Remove inactive B | Remove and Close share membership→lifecycle. | Either order; A closes and B is either retained or separately removed as explicitly requested. | Revalidate B and active relationship under the same gates. | No accidental removal/reorder of either member. |
| Close vs Remove A | Same membership→lifecycle gates. | Remove first sees active A and rejects `member_active`; Close first makes A inactive, then a later separate Remove may succeed. | Removal cannot slip between active check and Close publication. | No combined close-and-remove. |
| Close vs Connect | Connect start/publication uses lifecycle; pending work remains `Connecting`. | Connect start/publication first: Close rejects runtime busy. Close first while NotConnected: no old-A connect can be pending under current source; a later explicit Connect captures no active repository. Any test-seeded old capture fails repository-generation currentness. | Provider publication must match captured repo/model/profile/connection generations and `Connecting`; no stale A composition publishes. | Close never tears down or retries Connect. |
| Close vs Disconnect | Disconnect uses lifecycle and publishes `Disconnecting` before awaiting shutdown. | Disconnect first: Close rejects until it returns to exact `NotConnected` and provider activation is absent; if `HostRunning` still owns work after Disconnect, Close remains `repository_effect_busy` until that owner reaches its terminal boundary. Close first is possible only when already NotConnected; Disconnect then has no active runtime to stop. | There is no success while shutdown is in progress or failed/ambiguous. HostRunning cannot be erased by Disconnect or Close. | No implicit Disconnect or duplicate shutdown. |
| Close vs model-turn start/publication | Turn start/terminal ownership uses lifecycle and HostInvocation. | Existing turn wins: Close returns `model_turn_busy`. Close wins only in a coherent disconnected/no-turn state; a model turn cannot start against that state. | Any captured turn/runtime publication tied to A is stale if an artificial delayed publication is exercised. | No cancellation, migration, discard, or replay. |
| Close vs HostExplicit prepare/start | Prepare/finalize/Confirm/Cancel use lifecycle; effects hold HostRunning. | Close first clears a safe HostPrepared ticket; Confirm/finalize then fails. Confirm/start first changes state to HostRunning; Close rejects busy until terminal owner releases. | Ticket must still match composition and generation; it cannot survive/revive after Close. | No Tool dispatch from losing Confirm and no retry of Started operation. |
| Close vs Commit review/authorization/start | Capture/final Commit authorization publication uses lifecycle; Commit execution is model-turn-owned. | Close first invalidates the capture/review and clears pending authorization; final auth publication is stale. Auth publication first leaves a pending no-effect authorization for Close to clear. Commit start/model turn first blocks Close. | Existing repo generation/capability/review checks reject stale completion. | No commit, rollback, or replay from Close. |
| Close vs Stage reservation/start | Reservation/start entry uses lifecycle; reservation persists through attempt/completion. | Reservation first: Close returns `repository_effect_busy` without removing it. Close first invalidates selector; action then fails stale before reservation/native attempt. | Action must match repository and observation generations. | Close native Stage attempts = 0; no replay. |
| Close vs Unstage reservation/start | Same as Stage. | Reservation first: busy. Close first: stale action. | Action must match repository and observation generations. | Close native Unstage attempts = 0; no replay. |
| Close vs second Close | Both use membership→lifecycle. | First succeeds; second returns `no_active`. If requests contend before first publication, exactly one publishes Close. | Old guard cannot trigger another generation advance. | Second Close has no mutations/activity. |

## Closed result/error and privacy surface

The command has a closed result union:

| Outcome/error | Meaning |
| --- | --- |
| `closed` | One active member was intent-matched and the zero-active transition published. |
| `no_active` | Already in coherent zero-active state; no mutation or generation churn. |
| `active_changed` | Member/generation guard no longer matches; no mutation. |
| `connected_or_runtime_busy` | State is Connected, Connecting, Disconnecting, Error, or has provider ownership/publication. Disconnect first; no shutdown was attempted. |
| `model_turn_busy` | Model/chat/active-chat owner remains. No cancellation. |
| `repository_effect_busy` | HostRunning, Stage/Unstage reservation, or Commit-control exclusion prevents safe withdrawal. Existing owner is preserved. |
| `invalid_guard` | Malformed selector/currentness or unknown input field. |
| `unavailable` | Poison/invariant/generation-exhaustion or other internal bounded failure; no internal details or partial mutation. |

Internally, tests preserve the distinction between runtime, model-turn, and
repository-effect busy reasons. Frontend may provide a shorter bounded message
without exposing owner details.

Successful response fields are limited to:

```text
outcome: "closed"
activeMemberId: null
members: [{ memberId, displayName, active: false }]
status: "Repository closed. No repository is active."
```

The repeated-close response uses `outcome: "no_active"` and a bounded
no-active status. `memberId` is the existing opaque process-local selector
needed only for later explicit activation; `displayName` is the existing
bounded basename/label. Do not include membership or repository generation,
identity/currentness tuples, native/canonical paths, `.git`, filesystem/Git
identity, ToolRegistry/composition/provider handles, ticket, Commit review,
source content, or raw Tool input/output. Errors are bounded categories and do
not echo request values. Emit no Close Activity entry; the returned status and
explicit membership refresh are sufficient and repeated Close cannot create
Activity noise.

## Conversation, persistence, and non-effect guarantees

At successful publication:

- Call the existing in-memory `DesktopConversationState::start_new()` once.
  Its repository/model `identity` and replayable in-memory `history` become
  absent, and its existing epoch advances so a late completion cannot append
  into the closed context.
- Do not call persistence `clear`, `append_pair`, `append_separator`,
  `commit_resume_lineage`, or any migration. Do not write transcript bytes,
  delete a transcript, or change its namespace contents. Previously completed
  transcript bytes remain in their existing private repository namespace.
- Do not select a transcript namespace as an executable binding. Any later
  transcript presentation/read resolves the current namespace through the
  existing no-repository path. Durable transcript replay still requires
  explicit activation, connection, and the existing explicit Resume action.
- Do not read or write `remembered-workspace.json`. Candidate presence/order,
  labels, location hints, IDs, and `last_active_member_id` remain byte-for-byte
  unchanged.
- Keep global Trusted Profile selection/configuration unchanged and inert.
  With no active repository, it cannot create repository authority.
- Perform no repository file write/delete/move, `.git` mutation, Git status
  mutation, Stage/Unstage, HEAD/ref/history/branch operation, Commit, network
  Git, Tool dispatch, provider operation, or process/runtime shutdown. Close is
  process-local authority withdrawal. No repository filesystem validation is
  needed to withdraw in-memory authority; retained identity is freshly
  revalidated only at later explicit activation.

Restart semantics do not change. Restart yields zero admitted members, no
active repository/`DesktopRepository`, no repository-bound registry or
Effective Authority, no Commit/index/HostExplicit state, no runtime/provider
binding, and no executable in-memory conversation context. Descriptive
remembered candidates and completed transcript records remain under their
existing independent inert persistence contracts. Close itself is never
persisted as executable authority.

## Task 350 UI contract

When a repository is active, present an explicit `Close Repository` action and
human confirmation. Suggested wording:

> Close the active repository? It will remain admitted but inactive. Repository files and remembered workspaces will not be deleted. Disconnect first if a runtime is connected.

Bind the confirmation to the captured active member selector and
`repository.currentGeneration`; do not resolve current active state when the
human later presses Confirm. UI state is presentation only; backend repeats all
checks.

| UI state | Required presentation |
| --- | --- |
| Connected | Disable Close and show bounded Disconnect-first guidance. |
| Connecting or Disconnecting | Disable Close with bounded busy guidance. |
| No active repository | Hide or disable Close. |
| Model turn or active repository effect | Backend remains authoritative even if UI appears enabled; return bounded busy guidance and refresh. |
| HostExplicit ticket prepared | Existing explicit Cancel is required to permit Disconnect; do not make Close silently perform Cancel/Disconnect. |
| Successful Close | Show no active repository and retained inactive members; enable a separate explicit Activate action. |

Do not offer Close-and-Remove, Close-and-Disconnect, Close-and-Switch,
Close-and-Forget, or Close-and-Reconnect.

## Lifecycle acceptance matrix

`Invalidates` means deterministic no-effect process-local state removal;
`preserves` means no mutation; `rejects` means bounded zero-effect failure.
An effect owner always survives a rejected Close.

| State | Can Close encounter it? | Close disposition | Invalidate / preserve / reject | Generation/currentness effect | External-effect risk | Required deterministic test | Existing owner / reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Active member exists (A) | Yes; required success case. | `closed` if guards/preconditions match. | Retain A, set inactive, clear `DesktopRepository` and no-effect A state. | Repository generation +1; membership/admission generation unchanged. | None from Close. | A active → Close → same ID/member count, no active member/repository. | `WorkspaceMembershipState` + `DesktopAppState.repository`. |
| No active member | Yes; startup/repeated request. | `no_active` if coherent. | Preserve members and all state; zero mutation. | No generation churn. | None. | Repeat Close twice; assert all bytes/state/gens unchanged on second. | Existing valid zero-active state from ADR 0027. |
| Connected runtime | Yes. | `connected_or_runtime_busy`. | Reject; preserve runtime/providers/repository. | None. | Runtime/provider effects possible; Close does not touch them. | Connected real/test runtime Close rejects, still Connected and same provider owner. | `ConnectionState::Connected`, `provider_activation`. |
| Connecting runtime | Yes. | `connected_or_runtime_busy`. | Reject. | None. | Provider/runtime construction in progress. | Barrier after `begin_connect`; Close rejects; publication completes only under captured currentness. | `ConnectionState::Connecting`. |
| Disconnecting runtime | Yes. | `connected_or_runtime_busy`. | Reject until explicit shutdown returns. | None. | Shutdown is in progress/uncertain. | Barrier in Disconnect shutdown; Close rejects without second shutdown. | `ConnectionState::Disconnecting`. |
| Provider publication | Yes, only represented while Connecting before publication. | `connected_or_runtime_busy`. | Must already be absent for success. | Pending publish still bound to prior repository generation. | Providers/runtime can become externally active. | Pause before connected publication; Close rejects; no partial no-active state. | Local `PendingConnectedPublication` + Connecting and publication currentness. |
| Active model turn | Yes. | `model_turn_busy`. | Reject; retain terminal/chat owner. | None. | Captured A Tool/provider effects may still occur. | Start turn, barrier before terminal, Close rejects; no cancel/rebind. | `HostInvocationCoordinator::ModelTurn`, `chat`, `active_chat`. |
| Repository workflow observation | Yes, including async refresh after Disconnect. | Success may invalidate; stale refresh cannot publish. | Clear observation, actions, review. | Old generation invalid at Close; late final publication fails. | Read-only Git observation only; no mutation. | Barrier at final refresh publication vs Close; assert no post-close workflow install. | `refresh_repository_workflow` + workflow state; Task349 adds final gate. |
| Commit review prepared | Yes as review state; normal Disconnect may already clear it. | `closed`. | Clear review, selector, snapshot, authorization presentation. | Old repository generation invalid. | None before authorization/start. | Prepared review disappears; old review ID rejected after close/reactivation. | `RepositoryWorkflowState`. |
| Commit authorization current/pending | Yes defensively; normal Disconnect clears it first. | `closed` if control slot nonblocking clear succeeds; otherwise `repository_effect_busy`. | Clear pending no-effect authorization and drop capability/workflow. | Old generation/current capability cannot publish again. | Commit not started; model/execution owner is separately busy. | Seed armed pending slot; close clears; barrier auth preparation; final install stale. | `DesktopCommitCapability` + `RepositoryCommitControl`. |
| Commit effect started/uncertain | Yes only while its model-turn owner remains; current Commit is not separately reserved. | `model_turn_busy` or runtime busy. | Reject; preserve owner and do not clear authorization/effect state. | No Close increment. | Native Git history effect may have occurred. | Start model Commit lifecycle; Close rejects; one native attempt accounting unchanged. | Captured model turn owns `repo.commit`; not HostExplicit. |
| Stage prepared selector/action | Yes. | `closed`. | Clear selectors/action/review; no Git action. | Repository generation +1 makes any copied selector stale. | None; no reservation yet. | Refresh while disconnected, capture action ID, Close, action fails stale before native attempt. | `RepositoryWorkflowState.actions`. |
| Stage effect started/uncertain | Yes. | `repository_effect_busy`. | Reject; do not clear reservation. | No Close increment. | Index mutation may have happened; uncertainty cannot imply no effect. | Reservation/native-start barrier; Close rejects and token remains until normal owner resolution. | `repository_index_effect_reservation`. |
| Unstage prepared selector/action | Yes. | `closed`. | Clear selector/action/review. | Old generation invalid. | None; no reservation yet. | Capture Unstage action; Close; action stale before native attempt. | `RepositoryWorkflowState.actions`. |
| Unstage effect started/uncertain | Yes. | `repository_effect_busy`. | Reject; retain reservation. | No Close increment. | Index mutation may have happened. | Reservation/native-start barrier; Close rejects; no retry or cleanup. | `repository_index_effect_reservation`. |
| HostExplicit prepared ticket | Normally absent after Disconnect; defensive/synthetic state is possible in tests. | `closed` only when disconnected and no Started owner. | Clear ticket/preparation; pending finalize/Confirm fails. | Currentness changes with repository generation. | Zero effect by definition of HostPrepared. | Inject no-effect HostPrepared at close boundary; ticket cannot Confirm afterward; separately prove public Cancel-before-Disconnect flow. | `HostInvocationCoordinator::HostPrepared`; Disconnect currently refuses this state. |
| HostExplicit Started/uncertain/lost result | Yes while `HostRunning`. | `repository_effect_busy`. | Reject; leave coordinator/ticket execution owner intact. | No Close increment. | Tool effect may be in progress or unknown. | Hold HostRunning/Started barrier; Close rejects; release through existing terminal handling once, no replay. | `HostInvocationCoordinator::HostRunning`; deliberately conservative for reads too. |
| Activation publication | Yes; final transaction can contend with Close. | Serialize; one publication wins. | Close success retains A; loser stale. | Close increments repo generation; old transaction mismatches. | Constructed target is local until publication; no effect. | Barrier at activation final publication; assert allowed outcome and exact member/repository pair. | `membership_coordination` → `lifecycle_coordination`, `ActivationTransaction`. |
| Switch publication | Yes. | Serialize; switch-first gives `active_changed`; Close-first makes captured switch stale. | Preserve only winning active state. | One winning transition generation increment; no double publication. | No Git/provider effect from Close. | Barrier for A→B publication vs Close(A); verify no union. | Same activation transaction/publication path. |
| Admission publication | Yes. | Serialize under membership gate; either order valid. | B admission may be added but remains inactive. | Admission increments membership generation; Close does not. | Identity probe happens before membership commit; Close performs none. | Barrier before admission commit; verify A closed and B inactive in either order. | `admit_repository`, `membership_coordination`. |
| Inactive-member removal | Yes. | Serialize; independent removal may succeed. | Only explicit Remove changes B membership. | Remove increments membership generation; Close does not. | None from Close. | Race removal B vs Close A; assert only requested member removed. | `remove_repository_member_selector`. |
| Active-member removal (A) | Yes. | Removal-first rejects active; Close-first permits later separate Remove. | No combined mutation. | Close repo gen +1; later removal membership gen +1. | None from Close. | Barrier both orders; validate A retained on Close. | Removal active check under same gates. |
| Remembered candidate exists | Yes, independent. | `UNRELATED`. | Preserve catalog bytes and all descriptive values. | None. | Durable descriptive file only. | Hash `remembered-workspace.json` before/after Close. | `RememberedWorkspaceState` / ADR 0028. |
| Completed transcript exists | Yes. | `UNRELATED` durable; clear only current in-memory binding. | Preserve transcript bytes, namespace, records; do not auto-resume. | In-memory conversation epoch +1. | No disk write from Close. | Hash DB before/after; identity/history absent; explicit Resume still required after activation/connect. | `Persistence` vs `DesktopConversationState`. |
| Repeated Close | Yes. | `no_active`. | Preserve. | No generation churn. | None. | Assert exact state/generation/catalog/transcript equality and no Activity event. | No active member after first Close. |
| Reactivation after Close | Later, separate human action. | Not part of Close. | Retain member; explicit Activate builds fresh `DesktopRepository`; explicit Connect separately builds fresh composition. | Activation repository generation +1 again; membership generation unchanged. | Identity revalidation reads filesystem/Git as existing activation requires. | Reactivate A; current identity fresh, old selectors/tickets/reviews/snapshots stay stale; no transcript resume/reconnect. | `activate_admitted_member`, `ActivationTransaction`. |
| Restart | Close itself never persists executable state. | Existing restart contract. | Membership, active repo, registries, Commit/index/HostExplicit/runtime/conversation binding absent. Preserve inert catalog/transcripts. | All process-local IDs/currentness are new/absent. | Startup performs no candidate activation/provider reconnect. | Restart exact test; membership=0, repo/authority owners absent, durable descriptive bytes unchanged by Close. | ADRs 0027 and 0028. |

## Deterministic test plan for Tasks 349/351

Tests use disposable repositories and observable production backend boundaries.
Use explicit barriers/hooks at capture, reservation, and publication; no
sleep-based race tests.

### Membership and zero-active

- A active → Close → same A `RepositoryMemberId`, same membership count/order,
  inactive A, `active_member == None`, `repository == None`.
- A/B with A active → Close → both IDs/order/count unchanged and both
  inactive; no B activation.
- Close twice → first `closed`, second `no_active`; no second generation
  increment, state mutation, or Activity event.
- Prove `membership_generation` and A admission generation are unchanged by
  Close; prove member selectors remain usable only for explicit activation.
- Inconsistent no-active/member/repository state rejects `unavailable`, with
  no attempted automatic repair.

### Currentness and invalidation

- Successful Close advances existing repository generation exactly once with
  checked arithmetic; overflow rejects before all mutation. Active-selection
  generation remains absent; no durable epoch is added.
- Activation/switch and Close all reject repository-generation exhaustion
  before Commit revocation, membership publication, or repository withdrawal;
  no currentness increment wraps.
- Old Tool/Effective Authority repository snapshots cannot be used to acquire
  HostExplicit/dispatch authority after Close; disconnected state has no
  repository-bound registry or Effective Authority.
- Old Commit review and authorization cannot execute or republish; subsequent
  A activation requires fresh observation/review/authorization.
- Old Stage and Unstage selectors reject before native attempt; subsequent
  activation requires fresh observation/action selection.
- Old HostExplicit ticket rejects after Close/reactivation; no ticket moves to
  B or survives as executable. Test normal public ordering (Cancel prepared
  ticket → Disconnect → Close) and defensive no-effect HostPrepared
  invalidation separately.
- Delayed pre-Close activation/switch/provider/Commit-auth/workflow-refresh
  publications fail their existing active/repository generation and owner
  checks. Later explicit activation is a new capture, not stale replay.

### Preconditions and effects

- Connected, Connecting, Disconnecting, Error, or orphan `provider_activation`
  each reject without shutdown/retry. Exact NotConnected/no-provider case
  succeeds.
- Active model/chat/active-chat owner rejects; no cancellation or context
  rebinding.
- HostRunning, Stage/Unstage reservation, and Commit control-lock exclusion
  reject with the deterministic `repository_effect_busy` reason; the exact
  owner remains present.
- Hold a HostExplicit call in `HostRunning`, complete explicit Disconnect,
  and prove Close remains `repository_effect_busy` until that existing host
  owner reaches terminal handling; Disconnect does not clear it.
- `HostPrepared`, no-effect workflow/Commit review/authorization, and safe
  Stage/Unstage selectors are invalidated only in a successful publication;
  native effect counts remain zero.
- Stage, Unstage, HostExplicit, and Commit started/uncertain owner tests prove
  Close does not clear, mark success, retry, roll back, compensate, or replay.
- Run repository workflow observation with barriers before and after final
  publication; no captured A snapshot can install after Close.

### Race and isolation

- Barrier tests cover all 13 rows in the race matrix, including both allowed
  winner orders where both operations can publish. Assert bounded stale/busy
  outcome, zero loser effect, no partial state, and no replay.
- Close vs admission and inactive removal verifies independent membership
  effects and unchanged active target; Close vs active removal verifies active
  refusal or a later separate remove.
- Compare exact remembered catalog bytes and transcript DB bytes before/after
  Close. Compare repository file bytes, `.git` files, HEAD, refs, index,
  worktree/sentinels, and history before/after. Close counters must remain zero
  for Git/native mutations, filesystem mutations, provider actions, Tools,
  commits, reconnects, and model-selected Close actions.
- Keep B inactive and prove no B registry/union composition exists. Verify
  Trusted Profile configuration remains unchanged and inert.

### Privacy/result tests

- Serialize success, `no_active`, and every error; include sentinel native
  paths, canonical roots, `.git`, filesystem IDs, Git executable, tickets,
  provider/runtime handles, Commit review source, and Tool input in fixtures.
  Assert none appear in result/error/activity/log output.
- Success result has only outcome, no-active ID, member presentation using
  existing opaque member IDs and bounded basename/label, and bounded status;
  it contains no currentness generation.
- Close creates no Activity event. Repeated Close creates none.

## Task 352 Windows live-certification gate

Run against the exact reviewed/pushed source on Windows with a fresh disposable
real repository A and optionally B. Use certified Codex `0.149.0` only if
needed to establish the connected rejection case; submit no model prompt.
Never use the primary checkout as a destructive fixture.

1. Admit A (and optional B), explicitly activate A, and record sanitized member
   IDs/count/order, active repository generation/composition state, workflow,
   Commit, Stage/Unstage, HostExplicit, conversation, remembered-catalog,
   transcript, connection, provider, and runtime state.
2. If proving connected rejection, explicitly Connect Codex, verify exact
   connected runtime/composition state, call Close with captured A guard, and
   prove `connected_or_runtime_busy`, same active A, no provider/runtime
   shutdown, zero effects, and zero model turns. Explicitly Disconnect; prove
   shutdown/provider withdrawal completes and state is exactly
   `NotConnected` with no provider activation before continuing. If a prepared
   HostExplicit ticket exists, explicitly Cancel it before Disconnect.
3. Prepare only no-effect post-Disconnect state that production allows (for
   example repository observation and Stage/Unstage selectors). Exercise the
   defensive HostPrepared/Commit slot invalidation cases in deterministic
   backend tests if normal explicit Disconnect already removed/prevents them;
   do not misreport those seeded states as a reachable public live sequence.
4. Record hashes/snapshots of the repository tree, `.git`, HEAD, refs, index,
   worktree, sentinel bytes, transcript database, and remembered catalog.
5. Confirm Close A using the pre-dialog captured member+repository-generation
   guard. Prove A remains admitted/inactive, membership generation unchanged,
   active member none, `DesktopRepository` none, no repository-bound registry
   or Effective Authority, no Commit/index/HostExplicit executable state, no
   active conversation binding/history, global profile remains inert, and
   optional B remains independently inactive.
6. Prove repository/Git/filesystem bytes, transcript bytes, and remembered
   catalog bytes are unchanged. Close accounting is exactly:

   ```text
   Git mutations                    = 0
   filesystem mutations             = 0
   automatic commits                = 0
   automatic activations            = 0
   automatic provider reconnects     = 0
   model-selected Close actions      = 0
   Tool dispatches caused by Close   = 0
   ```

7. Repeat Close; prove bounded `no_active`, no generation churn, no Activity,
   and zero effect. Deterministic/live harness also proves a safely
   constructible Started/uncertain owner blocks Close without clearing or
   replaying it.
8. Explicitly activate A. Prove retained identity is freshly revalidated,
   `DesktopRepository` is new, repository generation/composition currentness
   is fresh, stale pre-Close Tool/Commit/Stage/Unstage/HostExplicit/runtime
   state stays dead, and no transcript auto-resume/provider reconnect occurs.
   If B exists, verify it is still inactive and no union registry exists.
9. Restart and prove zero process-local membership/active/repository/runtime/
   provider/registry/Commit/index/HostExplicit/conversation authority, while
   catalog and transcript remain inert and unchanged by Close.
10. Record the gate as host-driven production-backend evidence. Unless actual
    supported GUI automation ran, state exactly: `GUI automation was NOT
    EXECUTED`. Do not infer GUI certification from frontend static tests.

Preserve all Task 347 nonclaims, including no model-selected dynamic Tool
dispatch certification, no race-free TOCTOU, no network-isolation/OS-sandbox
claim, no rollback/replay/compensation, and no Linux/macOS live parity claim.

## Task 349 exact scope

**Task 349 — Production Zero-Active Repository Withdrawal Foundation**

Implement only:

- membership active→none primitive retaining the same member and membership
  generation;
- host-owned explicit Close lifecycle helper and exactly guarded input
  (`expected_active_member_id`, `expected_repository_generation`);
- existing membership→lifecycle coordination order and disconnected/provider
  precondition;
- actual chat/HostInvocation, index reservation, and Commit-control busy
  checks; no generic busy flag or new uncertainty ledger;
- deterministic invalidation of safe no-effect HostPrepared/workflow/Commit/
  Stage/Unstage state, with started/unresolved owners preserved and rejected;
- `DesktopRepository`/repository-bound authority withdrawal and exactly one
  checked increment of existing `repository_generation`, using the same
  checked publication helper from activation/switch;
- synchronization of late repository workflow publication with Close
  currentness;
- sanitized bounded backend result/error and deterministic Rust/backend tests.

Do not implement full frontend UX; no provider/runtime shutdown; no model/Tool
route; no new dependency; no Cargo or persistence/schema change; no permission
default enablement; no HostExplicit change; no ADR change. If a Tauri command is
registered, register only the exact Close command. Its generated permission
may exist, but do not enable it in the default frontend capability until Task
350.

## ADR disposition and validation boundary

ADR disposition: **No new ADR required.** ADR 0027 covers retained inert
membership, zero-active withdrawal, fresh active-only composition, and
repository-bound currentness. ADR 0028 remains unchanged; Close never changes
remembered descriptive workspace persistence. No source finding requires a
new authority category, persistent active intent, inactive-member executable
ownership, implicit provider shutdown, or schema change.

Task 348 deliverable is this document only. Required local validation:

```text
git diff --check
cargo metadata --no-deps --format-version 1
```

Metadata must report exactly 13 workspace packages, all version `0.28.0`,
Rust edition `2024`. `Cargo.lock` and dependencies stay unchanged. No Rust,
frontend, test, Tauri permission/default capability, generated permission,
ADR, schema, release gate, or other file may change. Before commit, inspect
the complete diff and require `git diff --cached --check`.

Preferred commit: `docs: define active repository close contract`.
Completion requires push to `origin master`, `HEAD == origin/master`, clean
worktree, and exact-head CI PASS.
