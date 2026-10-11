# Task 351 — Active Repository Close Security Audit

Status: **PASS WITH NARROW HARDENING** — deterministic source is ready for Task 352.

Parent checkpoint: `9ec679d3e3ab6fff7e07e0f157b268c81961f896`.
Task 350 exact-head CI: `35180299179` — PASS.
Audited production source: `99f44aebb89fb697573f16f40a849290d5a9cf09`
(`fix: harden active repository close lifecycle`).
The audit record is a docs-only descendant of that production source.

## Scope and source identity

This was an independent review of the production backend and frontend paths at
the parent checkpoint. Tasks 348–350 supplied the accepted contract and
implementation history; passing their tests was not treated as proof. The
review traced current owners, asynchronous publication paths, Close’s actual
mutation order, Tauri permission wiring, and the frontend invoke call graph.

The review found in-contract lifecycle and currentness defects. The fixes keep
authority classification **NONE**: no new capability, permission, Tool,
HostExplicit eligibility, provider route, persistence schema, or ADR was added.

## Ownership table

| State / owner | How it becomes current | What Close does | What blocks Close | What invalidates it | Can it publish late? | External-effect risk |
|---|---|---|---|---|---|---|
| Workspace membership and active member — `WorkspaceMembershipState` | Human admission creates an inert member; activation publishes one member under membership then lifecycle coordination. | Retains all members, IDs, order, and membership generation; publishes `active_member = None` last. | Malformed/unknown guard, changed active member, incoherent active member. | Explicit member removal or a later explicit activation; Close itself changes no membership generation. | Activation revalidates its captured member and repository generation at final publication. | None; membership is descriptive, not authority. |
| `DesktopRepository` and repository generation | Activation revalidates the member identity and constructs a fresh repository object; checked generation increment accompanies publication. | Removes the active repository object and advances the checked generation once. | Missing repository, root mismatch with active member, zero/stale generation, exhausted generation. | Close, switch, later activation, or existing capability-specific currentness invalidation. | Async work captures the repository Arc and generation and must revalidate before publication. | Repository object owns repository-bound Tools/authorities; Close itself performs no Git or filesystem operation. |
| Repository workflow, Stage/Unstage selectors, staged review | Async observation prepares outside the lifecycle gate, then rechecks repository Arc, active member, and generation before install. | Clears actions, review, commit review, selector, and authorization. | An acquired index-effect reservation blocks Close. | New observation, switch, Close, and checked observation/action-sequence exhaustion. | Yes; final publication is lifecycle-gated and currentness-checked. Counter exhaustion clears selectors and returns an unavailable review. | Stage/Unstage use existing reservation and native index owner; Close never dispatches them. |
| Stage/Unstage effect reservation | A current selector acquires one lifecycle-bound reservation carrying token, repository generation/member, and repository Arc. | Leaves the reservation untouched by rejecting as `repository_effect_busy`. | Any reservation, whether Stage or Unstage. | Only that effect’s existing completion/abort path; stale completion cannot clear a newer reservation. | Completion can arrive late, but must match the exact reservation/currentness tuple. | Started or uncertain index work stays owned; Close does not cancel or replay it. |
| Commit review, capability, and authorization | A current workflow review is captured and authorized against repository/model/identity/observation generations and the paired `RepositoryCommitControl`. | Clears a safely clearable pending authorization, capability, and workflow only after every mutation lock is held. | Model turn or Commit-control busy result. | Refresh, switch, Close, disconnect, identity/model change, or fresh review requirements. | Authorization final publication is lifecycle/currentness-gated; a losing publication cannot restore A. | Commit execution is never started by Close. Running/uncertain execution remains owned by the model turn. |
| HostExplicit preparation and running owner | Prepare reserves `HostPrepared` before async zero-effect preparation; Confirm consumes the ticket and owns `HostRunning` through terminal handling. | Clears safe `HostPrepared`; rejects `HostRunning` without changing its owner. | Any `HostRunning` owner, including an uncertain result. | Close/switch invalidates prepared tickets; only the existing terminal path releases a running owner. | Preparation finalization is lifecycle-gated and fails after Close clears the reservation. | Started or lost results remain conservatively owned. Close does not dispatch, cancel, or replay. |
| Runtime connection and provider activation | Connect first publishes `Connecting`, then publishes the runtime/composition only under lifecycle coordination and captured repository/model/profile/connection/identity currentness. Provider activation is stored separately. | Requires `NotConnected` and no provider activation; leaves both fields untouched. Close never calls Disconnect. | Connecting, Connected, Disconnecting, Error, or any provider activation in an impossible combination. | Existing connect/disconnect lifecycle transitions; later explicit Connect after Close captures no active repository. | Provider publication is gated and revalidates its captured tuple. A Connect reservation prevents Close from winning mid-flight. | Runtime/provider shutdown and cancellation are external lifecycle effects and are not part of Close. |
| Model/chat owner — `HostInvocationCoordinator`, `ChatState`, `ActiveChat` | `start_chat` reserves `ModelTurn`; runtime/session registration captures immutable connection/composition context. | Rejects while any of the three ownership signals is active. Does not call Cancel. | Model turn, non-idle chat state, or active chat record. | Existing terminal publication/finalization; uncertain started Tool work retains the owner. | Yes; terminal and session publication are generation/runtime/session-bound. | An unfinished started Tool result remains uncertain; its owner is retained rather than treated as no effect. |
| In-memory conversation | Chat captures its epoch; completed pairs publish only for that epoch. | Calls `start_new()` exactly once, withdrawing executable history/context. | Active model/chat owner. | Close or explicit new conversation/context change. | Completion may publish only while its model owner remains held through commit and transcript persistence. | Memory-only reset; no persistence mutation. |
| Effective Authority snapshot | Computed from current backend repository, workflow, runtime, profile, and Tool composition state. | After success, reports no selected repository, no repository kind/generation, disconnected, and no effective Tools. | Backend state remains authoritative even if the frontend snapshot is stale. | Any current backend state publication; refresh failure clears the frontend’s prior snapshot. | Snapshot readers see a coherent pre- or post-Close state through mutex synchronization. | Presentation only; it cannot grant authority. |
| Repository ToolRegistry/composition | Built from the freshly active `DesktopRepository` during explicit Connect and stored in the Connected composition. There is no inactive-member registry cache in `DesktopAppState`. | Does not operate on a connected composition; removes A only while runtime/provider is already absent. | Connected/runtime/provider state. | Disconnect or Close; later activation and Connect construct fresh current composition. | Connect publication is guarded by the captured currentness tuple. | No registry remains attached to inactive A and no A/B union is created. |
| Remembered-workspace catalog | Separate `RememberedWorkspaceState` and `remembered-workspace.json`; startup is descriptive and inert. | Does not read or write the catalog. | None. | Only explicit catalog actions. | No Close refresh path writes it. | No filesystem effect from Close. Byte snapshots remain equal. |
| Completed transcript persistence | Completed prompt/answer pairs are written through the existing persistence owner; persistence is separate from in-memory conversation. | Does not delete, clear, resume, or rewrite transcript data. | Active chat must finish terminal publication first. | Only existing explicit transcript/history actions. | A completion may finish persistence while Close is blocked by the retained model owner. | Close has no persistence call. Durable bytes remain equal across Close. |

## Close linearization and failure ordering

The production `close_repository_transition` now performs this order:

1. Parse the closed member selector and positive generation guard.
2. Acquire `membership_coordination`, then `lifecycle_coordination`, then the
   membership state. Poisoned locks return bounded `unavailable`.
3. Check that the member exists and is still the active member. Check that the
   current `DesktopRepository` exists and its root matches that member, and
   that the generation is positive and equals the captured generation.
4. Check model/chat ownership, Stage/Unstage reservation and HostRunning,
   runtime/provider disconnection, and checked next-generation arithmetic.
   No-active requests also fail closed on retained repository, Commit,
   workflow, reservation, host/chat, or runtime/provider state.
5. Acquire every mutation mutex before the final fallible operation, in this
   order: repository, repository generation, workflow, HostInvocation, Commit
   capability, conversation. Any poison failure leaves state unchanged.
6. Call the nonblocking Commit authorization clear. If it reports busy, return
   before repository, workflow, conversation, or membership mutation.
7. Synchronously clear safe HostPrepared, Commit capability, workflow, and
   repository state; publish the checked next generation; reset in-memory
   conversation state once; then set `active_member = None` as the final
   authority publication point.

There is no `.await`, filesystem or Git call, provider/runtime call, callback,
event, or serialization that can fail in the mutation region. Presentation is
built from the already-held membership guard after the final publication; it
does not reacquire a mutex. All rejectable state and lock acquisitions precede
authorization withdrawal. There is no wrapping generation update.

The code returns bounded `unavailable` for poisoned/incoherent state instead
of recovering a poisoned lock or panicking when an active member record is
missing. A connected state with zero repository generation is also treated as
incoherent in the no-active check; any Connected/Connecting/Disconnecting
runtime state prevents a false `no_active` result.

## Lock-order findings

| Path | Coordination and state-lock order | Finding |
|---|---|---|
| Close, activation/switch, inactive-member removal | `membership_coordination` → `lifecycle_coordination` → membership and currentness/state locks. | Same coordination order. No path was found taking membership coordination after lifecycle coordination. |
| Connect, Disconnect, chat start/terminal, HostExplicit prepare/confirm/cancel, Stage/Unstage reservation, Commit authorization | `lifecycle_coordination` → the operation’s short state locks; async work releases the gate and revalidates before publication. | Close serializes against each reservation/publication. No coordination gate is held across an await. |
| Workflow refresh final publication | lifecycle gate → repository/current member/generation checks → workflow installation. | Late A observation cannot install after Close or switch. |
| Effective Authority read | Workflow state precedes HostInvocation state. | Close’s final lock order uses the same workflow → HostInvocation relation. |
| Close final mutation locks | repository → repository generation → workflow → HostInvocation → Commit capability → conversation, under both coordination gates. | All locks are acquired before the last fallible authorization clear. No reverse acquisition was found in interacting production paths. |

Mutexes used by currentness readers are released before async work. Chat
conversation commit releases its state mutex before persistence and before
terminal finalization reacquires lifecycle coordination. No inversion or
blocking sleep is used as a race proof.

## Currentness and generation findings

- Activation/switch and Close use checked `repository_generation` advancement.
  Exhaustion fails without wrapping or mutation. Close advances exactly once;
  reactivation advances again and retains the same process-local member ID and
  unchanged membership generation.
- Workflow observation generation and selector sequence previously used
  unchecked increments. The publisher now checks both the observation advance
  and the complete action/review selector count before installation. On
  exhaustion it revokes authorization, removes selectors/reviews, and returns
  `ReviewUnavailable` without wrapping. A deterministic test covers both
  counters at `u64::MAX`.
- Stage/Unstage selectors bind repository and observation generations;
  Commit review/authorization binds repository, model, identity, and workflow
  generations; Connect binds repository/model/profile/connection/identity
  generations; HostExplicit capture is tied to current composition and its
  existing coordinator/ticket lifecycle.
- `membership_generation` is intentionally not a Close currentness axis and is
  unchanged by deactivation. No generation is made durable.

## Frontend stale-intent and snapshot-skew findings

`repositoryCloseGuard()` reads the active member ID from rendered membership,
not the selected member dropdown, and reads a positive safe-integer generation
from the rendered supported Effective Authority snapshot. Opening the dialog
copies both values into `pendingRepositoryClose`. Confirm consumes that frozen
pair and does not reread current frontend snapshots or retry `active_changed`.

The Node VM test runs the actual `status.js` handlers: it opens on A/17, changes
rendered membership/authority to B/18, confirms, and asserts the invoke request
still contains exactly A/17. The backend regression separately calls the real
`close_repository_transition` with A’s stale guard after B has become active;
it returns `ActiveChanged` and preserves B. Together these prove the frontend
payload and backend guard independently, without treating a frontend mock as
backend authority.

| Snapshot condition | Frontend behavior | Backend result/authority |
|---|---|---|
| Membership A with an old A generation | A may be presented as closable if the snapshot is otherwise supported. | Current generation comparison rejects the stale request. |
| Membership A while Effective Authority says no repository | Close is disabled. | Backend current member/repository checks remain authoritative. |
| No active member with an older authority generation | Close is disabled because there is no active member to capture. | Repeated Close returns `NoActive` without generation or membership churn. |
| Pending A confirmation after frontend/backend switch to B | Confirm submits the captured A guard and never retries. | Backend returns `ActiveChanged`; B and its repository Arc remain intact. |
| App status disconnected while authority is stale/reconnect-required | The current UI requires exact `not connected`; an unavailable status disables Close. | Even if an old request arrives, the backend’s live member/generation/runtime checks decide. |
| Effective Authority refresh fails | The rendered snapshot is cleared; stale generation cannot newly enable Close. | No frontend field grants or substitutes for backend authority. |

## Race matrix

| Race | Coordination owner and deterministic evidence | Loser result / final state |
|---|---|---|
| Close vs Switch A→B | `task349_close_for_a_never_closes_a_newly_active_b`; `task349_close_wins_against_activation_prepublication` uses a barrier at activation publication. | Stale Close gets `ActiveChanged`, or late activation loses its final currentness check. B remains current when switch wins. |
| Close vs Activate B publication | Same barrier-controlled activation test and checked publication tuple. | If Close wins, late activation cannot publish under the old active/generation tuple; a later explicit activation is fresh. |
| Close vs Connect | `task349_close_and_connect_currentness_follow_lifecycle_order` runs both orders through `begin_connect` and `publish_connected_provider_state`. | Connect reservation/publication first makes Close reject busy. Close first leaves no repository; later Connect captures the new zero-repository generation. |
| Close vs Disconnect | `task349_close_preserves_runtime_model_and_repository_effect_owners` exercises Disconnecting rejection; source inspection confirms Disconnect publishes Disconnecting under lifecycle coordination. | Disconnect first blocks Close. Close can win only when already NotConnected with no provider activation, so there is no old runtime completion to tear down; a later Disconnect is inert. |
| Close vs late workflow refresh | `task349_close_suppresses_late_repository_workflow_refresh_publication` blocks at the real final publication seam. | Late observation fails currentness; workflow remains withdrawn. |
| Close vs HostExplicit prepare | `task349_close_prevents_late_host_prepare_publication` uses a barrier at final ticket publication. | Close clears safe HostPrepared; late finalize returns Busy and cannot reinstall the ticket. |
| Close vs HostExplicit start/Confirm | `task349_close_and_host_confirm_follow_lifecycle_order` exercises Close-first ticket invalidation and Confirm-first HostRunning ownership. | Confirm-first makes Close return `RepositoryEffectBusy`; HostRunning remains owned. No dispatch is started by the losing Close. |
| Close vs Stage reservation | `task349_close_preserves_runtime_model_and_repository_effect_owners` acquires the production Stage reservation. | Close returns busy, retains the reservation, and succeeds only after the existing reservation path completes. |
| Close vs Unstage reservation | `task351_close_preserves_unstage_reservation_without_index_effect` acquires a real workflow selector and production reservation. | Close returns busy; member, repository Arc, generation, reservation token, and index bytes remain unchanged. |
| Close vs Commit authorization | `task349_close_wins_against_commit_authorization_publication` barriers authorization publication; `task349_close_clears_pending_no_effect_commit_authorization` covers safe pending authorization. | Late authorization cannot rearm. Safe pending authorization is cleared; active Commit/model ownership blocks Close. |
| Close vs model-turn/terminal publication | `task351_close_waits_for_chat_terminal_publication_to_finish` claims terminal, then commits conversation and persists before releasing the model owner. `start_chat` and Close share lifecycle coordination. | Close remains `ModelTurnBusy` through terminal publication. A Close-first send has no connected runtime/context to capture. Uncertain Tool ownership remains held. |
| Close vs second Close | `task349_close_withdraws_active_state_and_preserves_membership_and_persistence` performs first success and second `NoActive`; coordination gates serialize concurrent calls. | Exactly one generation advance; second call changes neither generation, membership, nor persistence. |
| Old frontend confirmation vs new active B | VM A/17 → rendered B/18 test plus `task349_close_for_a_never_closes_a_newly_active_b` through the production guard. | Invoke carries A/17; backend rejects it and B remains active. No retry. |

Disconnect ordering, chat start ordering, and a concurrent second Close cannot
produce a second authority publisher: their lifecycle gate serializes them.
Tests exercise the relevant reservation/state invariants rather than relying
on sleeps or scheduler luck.

## Focused security and lifecycle audits

### HostExplicit, Stage/Unstage, and Commit

- Idle HostExplicit state is unaffected. Safe `HostPrepared`, including async
  preparation, is invalidated; its late publication cannot restore a ticket.
- `HostRunning` always blocks Close because its coordinator does not classify
  every running Tool as effect-free. Lost or uncertain result is not treated as
  proof of no effect.
- Stage/Unstage selector state is withdrawn with workflow state. A reservation
  blocks Close and retains its exact token/owner. Close invokes neither index
  operation. Existing stale Stage selector evidence and the new Unstage
  reservation test cover the two action families.
- Commit review-only and safely pending authorization are invalidated. The
  nonblocking Commit-control clear is the last fallible operation. A running
  Commit remains under model/HostInvocation ownership; Close never executes,
  cancels, or retries it. Reactivation requires fresh review/authorization.

### Chat, conversation, and transcript

The terminal path previously released `ChatState`/ModelTurn at terminal claim
before publishing the completed conversation and transcript, while retaining
an `ActiveChat` record. That split owner state could leave Close permanently
busy after a completed chat and made frontend status disagree with backend
ownership. The fix retains the chat record, ChatState, and model owner until
conversation commit and completed-pair persistence finish; one generation-
checked finalizer releases all three together. Uncertain started repository
effects retain model ownership. Close does not call Cancel, resume history,
delete a transcript, or change persisted transcript bytes.

The Close/reactivate regression checks in-memory history reset, preserved
transcript/catalog bytes, fresh repository composition, disconnected runtime,
same member ID, unchanged membership generation, and fresh repository
generation. The late terminal test checks that Close is busy before publication
and succeeds after publication without changing the resulting transcript.

### Remembered state, Effective Authority, and registry

Close does not call any remembered-workspace method or transcript persistence
method. Existing byte snapshots cover the complete storage directory across
Close, including `remembered-workspace.json` and completed transcript data;
candidate IDs, labels, ordering, location hints, and presentation metadata
therefore remain unchanged. Existing startup tests still require zero restored
membership, active repository, registry/composition, Stage/Unstage, Commit,
HostExplicit, runtime, and provider authority.

After Close, the backend snapshot is `NoRepository`, repository selected is
false, kind is None, current generation is None, connection is NotConnected,
and effective Tool inventory is empty. Global configured profile information
may remain descriptive, but cannot compose repository authority. The registry
is owned only by a Connected current composition; no inactive-member registry
or A/B union exists. Reactivation constructs a fresh repository/composition.

### Repository/Git non-effect and privacy

The Close implementation contains no filesystem, Git, network, runtime,
provider, event, callback, or persistence call. Deterministic tests compare
`.git/index` and worktree bytes around Close/reservation rejection, and storage
bytes around successful Close. This is not a claim of live Windows
certification.

Close accepts only `expectedActiveMemberId` and
`expectedRepositoryGeneration`; errors and status are closed, bounded values.
The existing sentinel serialization test verifies no native/canonical path,
`.git` identity, Git executable path, filesystem identity, generation in
ordinary error text, HostExplicit ticket, runtime/provider handle, selector,
or source-bearing review detail is included. Frontend refresh failure clears
the old generation; Close does not put authority detail into DOM or activity.

### Tauri permission and frontend isolation

The generated `allow-close-repository` permission maps only to
`close_repository`. The default capability has exactly the one new allow
entry, with no wildcard, duplicate, unrelated command, deny inversion, or
provider/model surface. `tauri_permission_test.js` checks the exact chain.
HostExplicit remains the same exact 11-tool allowlist.

The actual frontend call graph is button → capture guard → confirmation →
`closeActiveRepository` → `close_repository`. Cancel invokes nothing. Close
success updates membership and clears repository presentation, then refreshes
Effective Authority, status, and transcript through read paths. It does not
call Disconnect, chat cancellation, Activate, Remove, chooser/admission,
remembered-candidate mutation, HostExplicit cancel/confirm, transcript delete,
or history clear. `active_changed` does not retry. VM behavior tests exercise
the invoke payload and resulting state; source checks are supplementary.

## Defects found and corrections

1. **Terminal owner publication gap:** chat terminal claim released ModelTurn
   and marked chat idle before conversation/transcript publication, while the
   active-chat owner remained. Added one finalizer after publication, and
   retain ownership for uncertain started repository effects. Added a direct
   close-vs-terminal-publication regression.
2. **Poison/incoherent Close handling:** Close recovered poisoned locks and
   assumed the active membership entry existed. It now fails closed on every
   lock it needs before mutation, pre-acquires its full mutation lock set, and
   returns `unavailable` for a missing active member or incoherent zero-active
   runtime/provider state. Added a poisoned-final-lock regression.
3. **Selector-generation wrap:** workflow observation/action selector counters
   advanced unchecked. Both are now checked before any selector publication;
   exhaustion revokes selectors/reviews/authorization without wrap. Added
   deterministic `u64::MAX` tests for both counters.
4. **Unstage reservation coverage:** Close already checked the common index
   reservation, but direct Close coverage existed for Stage only. Added a real
   Unstage selector/reservation test that proves Close has no index effect and
   preserves the owner.
5. **Authority snapshot and frontend refresh coverage:** added direct backend
   Effective Authority assertions for zero authority after Close and fresh
   disconnected composition after reactivation; added a UI regression proving
   failed authority refresh clears the old Close generation.
6. **Connect-order evidence:** strengthened the deterministic test to exercise
   both Connect-first (Connecting and Connected publication) and Close-first
   (later Connect captures no repository) orders.

No ADR contradiction or new-authority requirement was found. ADR 0027 and ADR
0028 remain unchanged. There is no persistence/schema change, dependency drift,
or v0.28 historical release-record change.

## Test quality review

- The critical backend assertions invoke production lifecycle/currentness
  functions, including `close_repository_transition`, the workflow publisher,
  and actual reservation/publication helpers.
- Barrier/channel-controlled publication tests force the late activation,
  HostExplicit preparation, Commit authorization, and workflow refresh seams.
  No sleep is used as synchronization.
- The frontend test executes `status.js` in a DOM stub and checks the exact
  command payload after rerender; the backend test separately exercises the
  real stale guard and preserves B.
- Existing source-string checks remain supplementary for closed permission and
  call-graph boundaries; behavior tests cover the authority-sensitive claims.
- The overflow test invokes the production workflow publisher and checks both
  stale-selector withdrawal and unchanged exhausted counters.
- Windows live fixtures/tests remain ignored under their existing opt-in
  environment gates; Task 351 does not claim Codex live certification, GUI
  automation, model-selected Tool dispatch, rollback, or OS sandboxing.

## Baseline and validation

- `cargo fmt --all -- --check` — PASS.
- `cargo check -p rah-desktop` — PASS.
- `cargo test -p rah-desktop -- --test-threads=1` — PASS, 310 passed, 14
  intentionally ignored.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` —
  PASS.
- `cargo build -p rah-desktop --release` — PASS.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js` — PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS.
- `node crates/rah-desktop/frontend/remembered_workspace_test.js` — PASS.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 workspace
  packages; all `0.28.0`; all edition 2024.
- Cargo manifests and `Cargo.lock` — unchanged; no dependency drift.
- `git diff --check` — PASS.
- HostExplicit — exactly 11; Codex baseline — `0.149.0`.
- ADR 0027/0028, persistence schema, and v0.28 release records — unchanged.

## Task 352 live-only questions

Task 352 must exercise this exact production source
(`99f44aebb89fb697573f16f40a849290d5a9cf09`) through the
Windows Desktop backend lifecycle with real disposable Git repositories and
verify the live host’s repository/worktree/index/ref and persistence byte
non-effects, Close → zero active state, explicit reactivation, disconnected
runtime/provider state, and retained transcript/catalog behavior. Reconfirm
the successful exact source identity in its live evidence. GUI automation or
model-selected Tool dispatch is a separate claim and must not be inferred from
backend certification.

No release preparation is authorized until Task 352 passes.
