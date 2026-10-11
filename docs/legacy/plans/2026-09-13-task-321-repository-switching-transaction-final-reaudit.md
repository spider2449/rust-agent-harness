# Task 321 - RAH v0.26 Repository Switching Transaction Final Re-Audit

Date: 2026-09-13

Audit mode: independent, deterministic, audit only

Audited head: `29bdb4d9d7dd286871de7e4b274c794ac01b71d6`

Direct parent: `82b64b3944e0d794779160e2708e7f0c8881e769`

## 1. Authoritative checkpoint

The checkout matched the supplied checkpoint before this document was
created. The branch is `master`; the worktree was clean. Task 319 audit head
was `932d5decb966e6f45196a24aaf5f306579ad628b`, with Verdict B and finding
F-319-1: activation invalidation was not atomic with successful publication.

The accepted architecture is ADR 0027, Workspace/Repository Identity and
Authority-Composition Boundary. The published release remains RAH v0.25.0:

- immutable source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`;
- annotated tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`; and
- GitHub Release: `387406579`.

No v0.26 live two-repository certification is claimed.

## 2. F-319-1 recap

The old ordering was:

```text
A active with meaningful Commit/workflow state
-> activate B
-> B passes early preparation
-> revoke A Commit/workflow
-> final B currentness/publication check fails
-> A remains active with its state destroyed
```

The concurrent form was:

```text
A active
-> B and C capture candidates
-> one candidate publishes
-> the loser performs pre-publication invalidation
-> loser publication fails
-> the winner's newly installed state may be destroyed
```

Task 320 moved invalidation after final validation and added deterministic
stale-target and loser-after-winner coverage. That correction does close the
original ordering inside the activation function. This re-audit finds a new,
separate coordination defect in the authorization and model-state writers.

## 3. Task 320 commit/change map

Production commits:

- `93cd67eaf047f00a515797092a5941108138799d` - `fix: make repository switch invalidation atomic`;
- `82b64b3944e0d794779160e2708e7f0c8881e769` - `fix: reset repository workflow once at activation commit`.

Documentation commits:

- `d4b9e76a08fd85e433e76c6103c39faed274890d`;
- `71695f1bb00b8f8ece4edb784a72f649ef0867a7`;
- `29bdb4d9d7dd286871de7e4b274c794ac01b71d6`.

Task 320 exact-head CI was run `34735579458`, PASS. The production change is
limited to `crates/rah-desktop/src/main.rs` and
`crates/rah-tools/src/repository_commit.rs`, plus its documentation. No
Cargo manifest or lockfile change occurred.

## 4. Final transaction ordering

The final source shape is:

```text
capture_activation_transaction
-> target member/admission identity capture
-> first target identity revalidation
-> repository construction
-> second target identity revalidation
-> pre-publication test barrier
-> membership + lifecycle final exclusion
-> target member/admission/private-binding revalidation
-> expected active member and repository-generation checks
-> chat/model lifecycle check
-> connection lifecycle check
-> HostInvocation lifecycle check
-> current Git executable and target identity revalidation
-> try_clear_authorization_now reservation
-> clear HostPrepared
-> remove old Desktop Commit capability
-> reset old workflow
-> publish active member/repository/generation
-> select persistence namespace
-> start one fresh conversation
```

`activate_admitted_member` at `main.rs:6069-6108` performs no active-state
mutation before `publish_activation_if_current`. The final gate at
`main.rs:5977-6065` holds `membership_coordination` and
`lifecycle_coordination`, then performs the final member, active-member,
generation, chat, connection, HostInvocation, Git, and identity checks before
calling `reserve_commit_revocation`.

`publish_active_repository` at `main.rs:5819-5859` returns no recoverable
`Result`. Its member assumption is protected by the caller's final
coordination and checked with a debug assertion plus an assertion. A panic is
not treated as a recoverable failed-switch path. There is no normal
member-not-found, currentness, lifecycle, identity, or repository-validation
failure after reservation begins.

## 5. Commit revocation seam audit

`RepositoryCommitControl::try_clear_authorization_now` is at
`crates/rah-tools/src/repository_commit.rs:604-618`. It is a public Rust host
control method, not a `Tool`, not serializable, and not model-visible. It
captures no authorization value and returns only a boolean. It calls the
existing pending `tokio::sync::Mutex::try_lock`; a busy lock returns `false`
without mutation, and a successful lock takes the pending authorization and
returns `true`. No await occurs while the pending guard is held.

The ordinary `clear_authorization` remains an async `lock().await.take()`
operation. `RepositoryCommitTool::compose` constructs exactly one shared
`Arc<Mutex<Option<ReviewedCommitAuthorization>>>` for the Tool and its paired
Control. Tool execution takes from that same slot once. The focused Commit
suite confirms one-shot execution, reviewed authorization, current-reviewed
authorization, stale-review refusal, and nonblocking busy behavior.

No new permission, schema, provider, replay, or reconstruction path was
introduced by the seam. The seam itself is conformant. Its caller
coordination is not.

## 6. Authorization re-arm race audit

The required no-re-arm invariant is not met.

`authorize_repository_commit_review` at `main.rs:6282-6357`, reached by the
public Tauri command `repository_authorize_commit_review`, reads the current
generation and clones the old `commit_capability.control` at lines 6286-6310.
It does not acquire `lifecycle_coordination`. It then awaits
`clear_authorization`, reads and removes the workflow review, awaits
`control.authorize_reviewed_snapshot`, and can finally set
`workflow.authorization = AuthorizedPending` at lines 6337-6346.

The unsafe sequence is:

```text
A active with a valid reviewed A Commit request
-> authorization route captures A control and A review
-> activation B passes every final check
-> try_clear_authorization_now() succeeds and removes pending A authorization
-> before capability removal/publication, authorization route completes
   authorize_reviewed_snapshot() and writes the old A authorization again
-> activation removes the Desktop capability and publishes B
```

The authorization route can hold the review across its async policy
authorization, so this is not eliminated by the short duration of the commit
point. The route's final workflow write can also occur after activation has
reset the workflow, leaving a stale `AuthorizedPending` presentation on B.
The old Tool is normally dropped with the old Desktop capability and the
active connected composition is absent during switching, so this audit found
no direct product Commit effect against B from this sequence. The pending
old-A state is nevertheless re-armed after the successful revocation
reservation, violating the explicit transaction invariant and leaving stale
repository-bound authorization state reachable through retained host control.

The same missing lifecycle exclusion exists in the model/Commit state writers:

- `set_model_configuration` at `main.rs:4721-4767` changes model generation and
  clears Commit/workflow state without `lifecycle_coordination`;
- `reset_model_preferences` at `main.rs:4841-4868` changes model state without
  that exclusion; and
- `set_commit_identity` at `main.rs:4791-4836` changes Commit identity,
  generation, capability, and workflow without that exclusion.

These paths can mutate state while activation holds its final gate, including
after revocation reservation. The normal lifecycle transitions for chat,
connection, HostInvocation prepare/confirm/cancel, and terminal ownership do
use `lifecycle_coordination`; the authorization/model/identity writers do not.

Smallest correction scope: make every Commit authorization writer and every
model/Commit identity state transition participate in the same final
activation coordination, or introduce an equivalent host-owned activation
reservation state that they must reject. The correction must preserve the
single shared pending slot and must add a deterministic writer-versus-switch
race test. No persistence, removal, public Tool, or authority expansion is
needed. ADR 0027 remains sufficient once this coordination gap is closed.

## 7. Commit execution overlap audit

The normal product composition does not permit an active model/runtime Commit
Tool execution to overlap switching. `repo.commit` is not HostExplicit
eligible; it is present only in the connected first-party composition. The
activation capture and final gate reject `Connecting`, `Connected`, and
`Disconnecting`, while `HostRunning` and `ModelTurn` are rejected by the
HostInvocation check. Connection publication uses the same lifecycle
exclusion and repository-generation tuple.

The Commit Tool takes the shared pending authorization before awaiting its
repository effect, and its control does not expose execution or replay
authority. I found no product path where a connected Commit effect can start
for A while B publishes. This is a pass for effect overlap, subject to the
authorization re-arm blocker above.

## 8. Stale-B preservation

`task_320_stale_target_after_final_preparation_preserves_active_state` at
`main.rs:16546-16656` uses real disposable Git repositories. It activates A,
installs A repository generation and workflow state with a Stage action,
installs reviewed Commit state and pending authorization, creates a
HostPrepared ticket, adds conversation history, and records the persistence
namespace. B is paused after candidate preparation and then its `.git` is
replaced before the final gate.

The test passes with `RepositoryMemberStale` and verifies unchanged A member,
root, repository generation, workflow/actions, Commit capability and pending
authorization, HostPrepared state, conversation epoch/history, and
persistence namespace. The ticket remains current and consumable in the
otherwise-current A state. This directly closes the old stale-B ordering.

## 9. Loser-after-winner preservation

`task_320_loser_after_winner_preserves_winner_state` at
`main.rs:16659-16735` admits A, B, and C, activates A, pauses C at the
pre-final-gate barrier, then lets B publish. Only after B publishes does the
test install meaningful B workflow/action, Commit capability and pending
authorization, conversation history, HostPrepared state, and persistence
namespace. C then enters its final gate.

C returns `RepositoryBusy`; B remains active at the same winning generation,
with workflow/actions, Commit capability/authorization, HostPrepared,
conversation, and persistence namespace unchanged. The test does not pause C
after invalidation; it pauses C before the final gate. This closes the
Task 319 evidence gap for the original loser ordering.

It does not exercise the independent authorization-writer re-arm sequence in
section 6, so it cannot clear the final verdict.

## 10. HostPrepared ordering

The final source revalidates member binding, expected active member and
generation, chat, connection, HostInvocation, current Git executable, and
target identity before `reserve_commit_revocation`. Only after reservation
does it call `clear_prepared` at `main.rs:6050-6054`.

The stale-B test confirms a failed final target proof leaves the A ticket
prepared. The retained prepared-confirm race has the two valid outcomes only:
Confirm wins and activation fails with A running, or activation wins after all
checks and invalidates the ticket before B publishes. No ticket authority
selects a repository.

## 11. Lifecycle race audit

The following remain correctly excluded by the shared final lifecycle gate:

- `HostRunning` and `ModelTurn` fail before invalidation;
- `Connecting`, `Connected`, and `Disconnecting` fail before activation;
- chat start, cancellation, terminal claim, and finish coordinate through the
  lifecycle mutex;
- HostInvocation prepare, confirm, cancel, and running transitions coordinate
  through it; and
- connection publication rechecks its captured repository/model/profile/
  connection generation tuple under it.

The independent model configuration, model reset, Commit identity, and
reviewed Commit authorization routes identified in section 6 do not honor the
same exclusion. Therefore the lifecycle audit is not conformant even though
the existing activation/connection/HostInvocation race tests pass.

## 12. Lock-order and deadlock audit

Activation's documented synchronous order is membership coordination, lifecycle
coordination, then host state locks. The final function follows that order for
membership, chat, connection, HostInvocation, Commit capability, workflow,
repository, generation, persistence namespace, and conversation transitions.
The activation commit point has no await. `try_clear_authorization_now` is
nonblocking and does not hold a guard across an await.

The connected publication and chat/HostInvocation transition paths acquire
lifecycle before their state locks. I found no blocking `std::sync::MutexGuard`
held across an await and no switch-specific reverse lock cycle.

The missing authorization/model/identity coordination is a linearizability
defect rather than a proven lock-order deadlock. Adding the smallest
correction must preserve the existing order and must not wait asynchronously
while a host state lock is held.

## 13. Successful-switch semantics

The nominal successful A-to-B test passes and the source performs one
synchronous commit sequence: reserve old pending Commit authorization, clear
HostPrepared, remove the old Desktop capability, reset workflow once, publish
B, increment repository generation once, select B's persistence namespace,
and start one fresh conversation.

`publish_active_repository` contains no second workflow reset. Persistence and
conversation helpers do not indirectly reset workflow. Failed and losing
activation paths in the deterministic suite perform zero workflow resets.

The success semantics are not accepted as a complete transaction guarantee
because an uncoordinated authorization writer can repopulate stale A
authorization/workflow state during the commit window.

## 14. Selector and active-only regression

Task 318's selector remains descriptive routing only. The public switch route
parses a bounded process-local selector and calls the single
`activate_admitted_member` helper. No path-based, Tool-based, model-selected,
provider-selected, or second simplified switch path exists.

Already-active selection returns before transaction capture and performs no
revalidation authority extension, Commit revocation, workflow reset,
HostPrepared clearing, generation increment, or conversation reset.

Inactive members retain only descriptive private admission records. They do
not retain `DesktopRepository`, ToolRegistry, Commit capability, workflow,
HostExplicit ticket, provider activation, or runtime state. Successful
composition remains active-only; no union registry or parallel active
repository was introduced.

## 15. Public-contract comparison

Comparing Task 319 audit head `932d5decb966e6f45196a24aaf5f306579ad628b`
with final head `29bdb4d9d7dd286871de7e4b274c794ac01b71d6` found the expected
narrow Rust host-only method `RepositoryCommitControl::try_clear_authorization_now`
and its implementation correction. There were no Tool changes, Tool schema or
status changes, `PermissionLevel` changes, Tauri IPC changes, frontend
changes, provider protocol changes, Trusted Profile changes, ADR changes,
dependency changes, or Cargo/lockfile changes.

The exact HostExplicit eligible set remains 11:

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

`repo.commit` remains HostExplicit-ineligible. Task 315 nested-boundary
behavior remains closed and membership remains supplementary to path
isolation.

## 16. Deterministic validation

All commands below ran on the final source before this document was created.

- `cargo fmt --check` - PASS.
- `cargo check --workspace` - PASS.
- `cargo test --workspace` - PASS. Desktop: 232 passed / 10 ignored;
  `rah-tools`: 286 passed; remaining workspace, integration, and doc-test
  suites passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` - PASS.
- `git diff --check` - PASS before documentation creation.
- `cargo metadata --no-deps --format-version 1` - PASS: 13 packages, all
  version `0.25.0`, Rust edition 2024.
- `cargo build -p rah-desktop --release` - PASS.
- `node --check crates/rah-desktop/frontend/status.js` - PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js` - PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` - PASS.
- `node crates/rah-desktop/tauri_permission_test.js` - PASS.

Focused final-head suites:

- Task 320 atomicity regressions: 3 passed, 0 failed.
- Repository Commit suite: 16 passed, 0 failed.
- Task 317 activation race suite: 11 passed, 0 failed.
- Task 318 selector/active-only suite: 1 passed, 0 failed.
- Task 315 nested-boundary suite: 5 passed, 0 failed.

These tests establish the corrected stale-B, loser-after-winner,
HostPrepared, lifecycle, selector, Commit seam, and nested-boundary behavior
that they cover. No existing test covers the authorization writer completing
between successful revocation reservation and old-capability withdrawal.

## 17. Limitations and nonclaims

This audit makes no claim of crash atomicity, filesystem race freedom, TOCTOU
elimination, OS sandboxing, network isolation, rollback, or replay safety
beyond the existing contracts. No live two-repository certification was run
for Task 321. Current master is not v0.26 live-certified. The v0.25 live
evidence remains historical and belongs only to immutable source
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.

Workspace membership persistence, active-member persistence, repository
removal IPC/UI, cross-repository mutation, connected switching, and parallel
active repositories remain absent. No product-scope work was performed.

## 18. Final verdict

F-319-1's original stale-B and loser-after-winner invalidation ordering is
closed by the Task 320 source and tests. The final transaction nevertheless
has a material authorization re-arm and lifecycle-coordination defect:
`authorize_repository_commit_review`, `set_model_configuration`,
`reset_model_preferences`, and `set_commit_identity` can mutate the old
Commit/model/workflow state without the activation lifecycle exclusion after
the final gate has begun. The required no-re-arm invariant is therefore not
proven and is false for the reviewed authorization route.

## 19. Exact next task

Task 322 remains blocked. The smallest next task is:

`Task 321-C - Serialize Commit authorization and model/identity state writers with repository activation finalization`

It must be correction-only, preserve ADR 0027 and active-only composition,
close the post-reservation authorization re-arm window, coordinate all listed
writers with activation, and add deterministic writer-versus-switch evidence.
It must not add persistence, removal, connected switching, new authority, or
live certification. After that correction, perform a fresh independent
Task 321 re-audit. Only a later Verdict A may authorize Task 322's remaining
product-scope decision.

Verdict B — CORRECTION STILL REQUIRED
