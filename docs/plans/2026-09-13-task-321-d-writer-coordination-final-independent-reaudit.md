# Task 321-D — RAH v0.26 Writer Coordination Final Independent Re-Audit

## 1. Authoritative checkpoint

| Item | Value |
|---|---|
| Audited current master | `36d0390eb70f45e39014959abd3f81720296cc30` |
| Direct parent | `ed328a49e66bb4d2bb95726968e604e07af40dd6` |
| Task 321 audit head | `eff11d72ebe8856cb8e6898ff182272814ad1e6b` |
| Task 321 verdict | Verdict B — CORRECTION STILL REQUIRED |
| Task 321-C production commit | `f1602aceec9f0382fdb622c820b97864e781b92d` |
| Task 321-C documentation commits | `fdc24c2`, `ed328a49e66bb4d2bb95726968e604e07af40dd6`, `36d0390eb70f45e39014959abd3f81720296cc30` |
| Task 321-C exact-head CI | `34745348057` — PASS |
| Task 321-C outcome | Outcome A — WRITER COORDINATION CORRECTION CLOSED |
| Task 320 | ACTIVATION INVALIDATION ATOMICITY CLOSED |
| Accepted architecture | ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary |
| Published release | RAH v0.25.0 |
| Immutable v0.25 source | `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` |
| Annotated tag object | `ea3c31aaf5190b632d7ef86387f7aff6004ae664` |
| GitHub Release | `387406579` |
| Workspace baseline | 13 packages, version `0.25.0`, Rust edition 2024 |
| HostExplicit eligible set | Exactly 11 names |

This is an audit-only document. The audited source and tests were not changed.
The only intended change in this task is this document.

## 2. Task 321 blocker and independent findings

Task 321-C correctly closes the original authorization re-arm race. It does
not, however, establish one safe lifecycle ordering for every requested writer.
Two material residual defects were found in the current source.

### F-321-D-1 — Connect omits Commit identity generation from final publication

`connect_codex` captures `identity_generation` and embeds it in the pending
`DesktopCommitCapability` at `crates/rah-desktop/src/main.rs:6998-7016`.
`PendingConnectedPublication` has no identity-generation field, and
`publish_connected_provider_state` compares only repository, model, profile,
and connection generations at `:1933-2048`. The final publication therefore
does not revalidate the identity tuple that created the pending Commit
capability.

The reachable order is:

```text
Connect captures repository A / model M / identity I
-> Connect prepares capability bound to I and remains Connecting
-> set_commit_identity takes lifecycle_coordination
-> identity is persisted, identity generation increments, old capability/workflow withdraw
-> Connect final publication takes lifecycle_coordination
-> repository/model/profile/connection tuple still matches
-> stale capability bound to I is installed
```

`set_commit_identity` is allowed while `Connecting`, and its lifecycle gate
does not prevent this order. The stale capability is not merely descriptive:
it owns the Commit tool/control that can later arm the shared pending slot.
This violates the required “writer wins makes Connect stale” and “Connect
publishes the captured repository/model/identity tuple” properties.

The smallest correction is to carry the captured Commit identity generation in
`PendingConnectedPublication` and require it at the same final lifecycle gate,
dropping the unpublished capability and rejecting the stale Connect. A barrier
test must exercise identity change between Connect preparation and publication.
This is an implementation correction within ADR 0027; no ADR amendment is
indicated.

### F-321-D-2 — Desktop Stage/Unstage effect is outside lifecycle coordination

`repository_index_action` at `crates/rah-desktop/src/main.rs:6565-6668`
captures the selected repository/generation and validates `target_is_current`,
then awaits the Git Stage/Unstage tool without `lifecycle_coordination`.
Activation final publication at `:6050-6134` uses the lifecycle gate but does
not reserve or reject an in-flight index action. Consequently:

```text
A index action passes target_is_current
-> B activation publishes under lifecycle_coordination
-> A Git Stage/Unstage executes using the retained A repository
```

This is a real repository-authority ordering defect when selection is otherwise
allowed (the index action is not represented by `host_invocation`). It permits
an A index mutation after B is active and is not repaired by the subsequent
refresh, which observes the current repository rather than proving the A
effect was current. The smallest correction is a lifecycle-aware operation
reservation/currentness protocol for Stage/Unstage that does not hold a
blocking synchronous guard over `.await`, followed by current-only workflow
refresh. This also needs a deterministic A-action-versus-B-activation barrier
test. It is within the ADR 0027 authority boundary.

Because either finding is material, this audit cannot clear Task 322.

## 3. Task 321-C change map

The production diff is limited to `crates/rah-desktop/src/main.rs`,
`crates/rah-tools/src/lib.rs`, and
`crates/rah-tools/src/repository_commit.rs`.

The correction adds opaque prepared authorization and synchronous final
installation; serializes model configuration, model reset, and Commit identity
writers with `lifecycle_coordination` and `preference_ordering`; reserves
Commit revocation before authoritative writer transitions; and moves Commit
capability into `PendingConnectedPublication`. The change preserves the single
active composition, one shared pending slot, active-only membership, the exact
11-name HostExplicit set, and the existing public Tool contracts.

The moved Connect capability is the source of F-321-D-1: it is prepared before
the final gate, but identity generation is not part of that gate.

## 4. Two-phase authorization audit

The current Desktop path at `:6426-6534` has the required three logical stages:

1. Under `lifecycle_coordination`, it captures repository, model, Commit
   identity, workflow observation, review selector, and exact control identity;
   it synchronously clears the old pending approval and takes the opaque
   semantic review binding.
2. It releases the lifecycle guard before
   `prepare_reviewed_authorization(...).await`. Preparation validates the
   reviewed repository state and does not touch the shared pending slot.
3. It reacquires `lifecycle_coordination`, checks the captured generations,
   connection state, workflow observation/selector, current review, and exact
   `Arc` control identity, then calls the consuming synchronous installer. Only
   a successful install sets `AuthorizedPending`.

The final check rejects repository, model, identity, Disconnecting, capability,
review, selector, and observation staleness. `PendingSlotBusy` and
`ControlMismatch` do not publish workflow state. A preparation failure writes
`ReviewStale` only when the original capture is still current.

This closes the original A-preparation/B-activation re-arm race. It does not
close F-321-D-1 or the independent Stage/Unstage effect race.

## 5. Prepared candidate authority analysis

`PreparedRepositoryCommitAuthorization` has private policy and authorization
fields, no public arbitrary-value constructor, no `Serialize` or `Deserialize`,
no `Clone`, no `Tool` implementation, no `ToolInput`, no `PermissionLevel`, no
provider exposure, no model-visible path, no persistence, and no logging of
its internals. `PreparedRepositoryCommitAuthorizationError` is a bounded,
host-only classification.

`try_install_prepared_authorization_now` consumes the candidate by value. It
checks both exact `Arc::ptr_eq` policy identity and policy generation before
attempting a nonblocking pending-slot lock. A successful install moves the
authorization into the slot. `ControlMismatch` and `PendingSlotBusy` consume
the candidate and leave the slot unchanged. There is no retry/replay API,
candidate queue, or clone-and-retry path. The candidate is non-authoritative
until installed into the originating control's slot.

`RepositoryCommitTool` and `RepositoryCommitControl` are paired at composition
and share exactly one `Arc<Mutex<Option<ReviewedCommitAuthorization>>>`.
Tool execution consumes from that same slot. There is no second executable
pending store or candidate-owned authority channel.

The focused current-head `rah-tools` tests pass: 18 repository-Commit tests,
including exact-control mismatch, unarmed preparation, busy installation, and
single-use behavior.

## 6. Legacy authorization API call-site audit

The public host methods `authorize_reviewed_snapshot`,
`authorize_current_reviewed_snapshot`, and `clear_authorization` remain in
`RepositoryCommitControl`.

Workspace-wide search found no production Desktop/provider/runtime route that
uses either legacy arming method. Their observed callers are test-only:
`rah-tools/src/repository_commit.rs` tests, the Desktop Commit tests, and
`rah-runtime-codex/src/bridge_tests.rs`. The product Desktop route uses only
`prepare_reviewed_authorization` and
`try_install_prepared_authorization_now`; `clear_authorization` remains a
host invalidation seam used for cleanup/refresh. No legacy production bypass
was found. This is a call-site result, not a claim that the public legacy
methods have been removed.

## 7. Activation race audit

Task 320 final activation still validates the target member and expected active
member/repository generation, checks connection/chat/HostInvocation state,
reserves Commit revocation before the infallible commit point, clears prepared
HostExplicit state, withdraws Commit capability and workflow, publishes the
new active repository, increments repository generation once, switches the
persistence namespace, and starts a new conversation.

The Task 321-C barrier test proves A authorization preparation cannot re-arm
after B wins: A pending authorization is absent, B capability is absent while
disconnected, B workflow remains unchanged, and the resumed writer returns
`CommitAuthorizationStale`. The three Task 320 atomicity tests also pass.

The activation path remains sound for its covered races, but F-321-D-2 shows
that Stage/Unstage is an uncoordinated direct competitor to this lifecycle
transition.

## 8. Model, reset, and identity writer audit

`set_model_configuration`, `reset_model_preferences`, and `set_commit_identity`
all acquire `lifecycle_coordination` before `preference_ordering`, validate
input, call `reserve_commit_revocation`, perform the existing preference/state
operation, and withdraw Commit capability/workflow after a successful
authoritative transition. Model generation increments only when the effective
selection changes. Reset uses `apply_model_selection`, so it can increment
model generation when the default differs; same-selection reset is safe and
still fail-closed because it revokes pending Commit state and withdraws the
current capability/workflow. Commit identity generation increments once per
successful identity update.

Against authorization, these writers serialize with final publication and make
the candidate stale before any new capability/workflow is installed. Against
activation, both writers and activation take the same lifecycle exclusion, so
the covered transitions have one winner and no half-published model/identity
state. However, the identity writer can win while Connect is still preparing,
and Connect's final gate omits identity generation. F-321-D-1 therefore makes
the overall identity-writer verdict nonconformant.

Preference-save failure semantics are fail-closed:

| Writer | Before save failure | On save failure |
|---|---|---|
| Model configuration/reset | Pending Commit has already been revoked; current model is changed only by `apply_model_selection` before the save; capability/workflow remain present until the final withdrawal. | Model authoritative state remains the requested state, save warning is emitted, capability/workflow are withdrawn, and no old pending authorization remains. |
| Commit identity | Pending Commit has already been revoked; identity and generation are unchanged until persistence succeeds. | `CommitIdentitySaveFailed` is returned; old identity/generation/capability/workflow remain coherent, but the old pending authorization is safely lost. |

No failure combination falsely presents old authorization as valid under a
changed model or identity.

## 9. Connect and Disconnect audit

Disconnect takes `lifecycle_coordination`, changes the connection to
`Disconnecting`, removes the Desktop Commit capability, and resets workflow
before releasing the gate. It then clears the retained control's pending slot
with nonblocking `try_clear_authorization_now` or asynchronous clear after the
gate is released. A prepared writer that resumes sees missing capability,
Disconnecting/stale state, or exact-control mismatch and cannot publish. The
existing Disconnect test passes. No reachable product path was found that can
execute through the old pending slot during the clear window: the current
capability is absent, the connection is Disconnecting, and new Commit
publication is rejected.

Connect now carries `commit_capability` inside `PendingConnectedPublication`,
and the final gate publishes connection, provider, composition, allowed
permissions, and Commit capability together. This prevents a stale Connect
from installing capability before its other publication state. Repository,
model, profile, and connection currentness are checked. Identity currentness
is not checked, producing F-321-D-1. No final-head deterministic test covers
Connect versus Commit identity or Connect versus identity writer.

## 10. Preference lock-order audit

The modified writer order is:

```text
membership_coordination -> lifecycle_coordination -> preference_ordering -> authoritative state locks
```

The three modified model/identity writers use lifecycle then preference. The
two other production `preference_ordering` acquisition sites are trusted
profile preference save/forget paths; they do not acquire lifecycle in reverse.
No reachable `preference_ordering -> lifecycle_coordination` blocking order
was found, and no helper called while both are held acquires them in reverse.

The modified writers perform bounded synchronous preference I/O while holding
the lifecycle gate. This is an availability cost of the existing design, not
by itself an authority defect. No blocking guard crosses `.await` in the
modified authorization, activation, Connect, Disconnect, or writer paths.

## 11. Deadlock audit

The checked paths were activation, Connect final publication, Disconnect,
authorization capture/failure/final publication, model configuration, model
reset, Commit identity, workflow refresh, Commit review/authorization,
repository index action, and chat/model lifecycle transitions.

The lifecycle/preference order has no material reverse cycle. Pending-slot
operations use `try_lock` for bounded writer reservation/installation and do
not block while lifecycle is held. Disconnect releases lifecycle before its
async fallback clear. Authorization releases lifecycle before async review
preparation. Capability and workflow locks are acquired in short synchronous
sections; no nested reverse capability/pending or workflow/capability blocking
cycle was found.

The Stage/Unstage defect is an authority/currentness race rather than a
deadlock. It remains a required correction because the operation is not
serialized with activation.

## 12. Workflow and Stage/Unstage currentness audit

Workflow refresh increments observation generation, clears stale selectors and
reviews, and installs new review metadata only after the captured repository
generation remains current. Authorization binds repository generation,
observation generation, selector, and opaque semantic review; a refresh during
preparation therefore makes final publication stale. The existing refresh and
review invalidation tests pass.

The underlying Stage/Unstage tools independently retain repository nested-boundary,
target identity, and Git snapshot checks. The current Desktop action wrapper,
however, is not lifecycle-coordinated across its asynchronous effect. A
workflow/action selector check does not prevent activation between the check
and the native effect. This is F-321-D-2 and requires a focused correction and
barrier evidence.

## 13. Active-only authority audit

Membership remains descriptive and process-local. Inactive members retain no
Commit capability, workflow, prepared candidate, ToolRegistry,
DesktopRepository, HostExplicit state, runtime, provider activation, or
conversation authority. Activation creates one fresh repository-bound
composition and invalidates the previous one. No union registry, model/provider
repository selection, persistence of membership authority, or repository
removal was added.

Task 318 active-only selector/switching coverage passes. Task 315 nested-boundary
coverage remains independently enforced in the repository tools; membership
catalog isolation is not used as a substitute for path isolation.

## 14. Public-contract comparison

The expected host-only additions are present:

- `PreparedRepositoryCommitAuthorization`;
- `PreparedRepositoryCommitAuthorizationError`;
- `prepare_reviewed_authorization`;
- `try_install_prepared_authorization_now`.

They are re-exported from `rah-tools` because `rah-desktop` consumes the
host-side API. Their fields and policy binding remain private, with no generic
constructor, serialization, Tool visibility, provider exposure, or model
surface.

No change was found to Tool names/descriptions/schemas, status enums,
`PermissionLevel`, provider protocol, Trusted Profile schema, repository
selector input, or HostExplicit authority semantics. `repo.commit` remains
Execute permission but HostExplicit-ineligible. The exact eligible set remains:

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

No public-contract drift was found. The currentness defects are internal
authority-lifecycle defects.

## 15. Deterministic evidence

The final-head tests provide structural/deterministic evidence as follows:

| Requested matrix item | Evidence at this head |
|---|---|
| Authorization preparation loses to activation; stale A cannot re-arm; B workflow unaffected | Direct barrier test `task_321_c_authorization_preparation_cannot_rearm_after_activation` — PASS |
| Authorization publication wins before later activation revokes it | Task 320 successful-switch test plus authorization/Commit tests — PASS for covered ordering; no dedicated reverse barrier |
| Authorization versus model configuration/reset and Commit identity, both orders | Lifecycle/generation source proof; no direct final-head barrier matrix for these writer pairs |
| Authorization versus Disconnect, both orders | Disconnect test and lifecycle source proof — PASS for covered path; no dedicated reverse barrier |
| Model configuration/reset/identity versus activation | Shared lifecycle source proof; no direct final-head barrier matrix |
| Pending-slot busy has no partial writer state | `prepared_authorization_busy_install_does_not_publish_or_wait` and `try_clear_authorization_now_is_nonblocking_and_preserves_busy_state` — PASS |
| Exact control mismatch and one-shot candidate | `prepared_authorization_is_unarmed_bound_and_single_use` — PASS |
| Task 320 stale-target, loser-after-winner, successful revocation | 3 focused tests — PASS |
| Task 317 lifecycle races | Full Desktop suite and focused `activation` command — PASS; focused command was 12 tests including retained activation, provider, membership, and Task 321-C cases |
| Task 318 selector/switch | 1 focused test — PASS |
| Task 315 nested boundary | Focused `rah-tools nested`: 20 unit tests plus retained integration targets; all PASS |
| Exact HostExplicit 11 | Full Desktop authority tests and `selected_repository_registry_has_only_the_intended_bounded_tools` — PASS |
| Connect versus Commit identity | No barrier test; source defect F-321-D-1 is directly observable |
| Desktop Stage/Unstage versus activation | No barrier test; source defect F-321-D-2 is directly observable |

The absence of direct writer-pair tests would require documentation only if the
source invariants were complete. Here the source invariants are incomplete, so
the two material findings independently require Verdict B.

## 16. Validation

All commands below were run against the audited source head before adding this
document, except the final document diff checks recorded at closure.

- `cargo fmt --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace -- --test-threads=1` — PASS. Desktop: 233 passed,
  10 ignored; `rah-tools`: 288 passed; `rah-runtime-codex`: 83 passed,
  1 ignored; remaining workspace, integration, and doc tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages,
  version `0.25.0`, Rust edition 2024.
- `cargo build -p rah-desktop --release` — PASS.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js` — PASS.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.
- `cargo test -p rah-tools repository_commit -- --test-threads=1` — PASS:
  18 passed, 0 failed.
- `cargo test -p rah-desktop task_321_c -- --test-threads=1 --nocapture` —
  PASS: 1 passed, 0 failed.
- `cargo test -p rah-desktop task_320 -- --test-threads=1` — PASS: 3 passed,
  0 failed.
- `cargo test -p rah-desktop task_318 -- --test-threads=1` — PASS: 1 passed,
  0 failed.
- `cargo test -p rah-desktop activation -- --test-threads=1` — PASS: 12
  passed, 0 failed (the filter includes related activation/provider,
  membership, and Task 321-C tests).
- `cargo test -p rah-tools nested -- --test-threads=1` — PASS: 20 unit tests
  passed, 0 failed; retained integration targets selected by the filter also
  passed.
- `git diff --check` — PASS before document creation; rerun at closure.

No Cargo manifest or `Cargo.lock` change occurred. Dependency drift was not
observed.

## 17. Limitations and nonclaims

This audit makes no claim of crash atomicity, live two-repository certification,
filesystem race freedom, TOCTOU elimination, rollback, replay safety beyond
the existing contracts, OS sandboxing, network isolation, or cross-platform
parity. No v0.26 live certification was run. v0.25 live evidence remains
historical and belongs only to immutable source
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.

Prepared authorization, active member state, membership, Commit pending state,
and repository switching authority remain process-local and non-persistent.
Repository removal is absent. No release preparation was started.

## 18. Final verdict

F-321-D-1 is a material stale Commit-capability publication path: Connect's
final currentness gate omits Commit identity generation. F-321-D-2 is a
material uncoordinated Desktop Stage/Unstage effect path that can execute
against repository A after repository B is active. The corrected authorization
prepare/install path itself is conformant for the covered activation race,
candidate binding, one-shot behavior, and legacy product call-site audit, but
the requested whole-lifecycle ordering is not conformant.

## 19. Exact next task

Task 322 remains blocked. The smallest focused correction task is Task 321-E:

1. add Commit identity generation to Connect pending publication and final
   currentness, with a deterministic Connect-versus-identity barrier test; and
2. serialize Desktop Stage/Unstage effect and current-only refresh with
   activation, with a deterministic action-versus-activation barrier test.

Preserve ADR 0027, active-only composition, the single Commit pending slot,
the exact HostExplicit set, Task 320 atomicity, Task 317 lifecycle races,
Task 318 switching, and Task 315 nested-boundary enforcement. Do not add
persistence, member removal, connected repository migration, release work, or
live certification in the correction. Only a later independent re-audit may
unlock Task 322.

Verdict B — CORRECTION STILL REQUIRED
