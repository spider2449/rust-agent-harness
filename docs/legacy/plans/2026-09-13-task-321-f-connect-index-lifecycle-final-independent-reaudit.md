# Task 321-F - RAH v0.26 Connect and Index Lifecycle Final Independent Re-Audit

## 1. Authoritative checkpoint

| Item | Value |
| --- | --- |
| Audited master | `c93474ae14034ff6164ece7ed636ab6d398ef8a2` |
| Direct parent | `2b83bc056af07ccf15a0c19cf615dac2a4409d21` |
| Task 321-E production commit | `c93474ae14034ff6164ece7ed636ab6d398ef8a2` |
| Task 321-E exact-head CI | `34749268600` - PASS |
| Task 321-D verdict | Verdict B - CORRECTION STILL REQUIRED |
| Accepted architecture | ADR 0027 - Workspace/Repository Identity and Authority-Composition Boundary |
| Published release | RAH v0.25.0 |
| Immutable source | `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` |
| Annotated tag object | `ea3c31aaf5190b632d7ef86387f7aff6004ae664` |
| GitHub Release | `387406579` |
| Workspace baseline | 13 packages, version 0.25.0, Rust edition 2024 |
| HostExplicit eligible set | Exactly 11 names |

This is an audit-only result. No production code, test, frontend, permission,
Cargo, or ADR file was changed by Task 321-F. The only intended checkout
change is this document.

## 2. Task 321-D findings

F-321-D-1 was the missing Commit identity generation in Connect's final
publication currentness proof. F-321-D-2 was the absence of a lifecycle
reservation around asynchronous Desktop Stage/Unstage effects. Both findings
were material because the old paths could publish or execute authority after a
writer or repository transition had won.

Task 321-E closes F-321-D-1 in the ordinary Connect path and substantially
closes F-321-D-2 for activation and the listed lifecycle writers. A separate
material Connect-versus-index reservation gap remains; see section 10.

## 3. Task 321-E exact production change

The production diff from the pinned Task 321-D parent is limited to
`crates/rah-desktop/src/main.rs`; the other changed file in the Task 321-E
commit is its correction document. The implementation adds a five-component
Connect currentness value, stores identity generation in connected state and
pending publication, and adds one Desktop-private process-local index-effect
reservation. No Tool name, Tool schema, PermissionLevel, provider protocol,
Trusted Profile schema, Tauri IPC, frontend, Cargo dependency, ADR, or
HostExplicit eligibility changed.

## 4. Connect identity currentness audit

`ConnectionPublicationCurrentness` at `main.rs:1943-1957` contains repository,
model, Trusted Profile, connection, and Commit identity generations.
`connect_codex` captures all five before constructing the Commit capability,
first-party registry, provider composition, and runtime connection. The
capability's `identity_generation` is copied from the captured value, and
`PendingConnectedPublication` carries it explicitly at `main.rs:1991-2003`.

`publish_connected_provider_state` acquires `lifecycle_coordination` and
rejects a non-Connecting state or any mismatch in the complete five-field
tuple before installing connection state, provider activation, composition,
permissions, or Commit capability. Rejection returns the runtime and provider
activation for cleanup; no partial Connected state, stale registry, or stale
provider owner is installed.

The post-runtime check in `connect_codex` also compares identity generation.
It is an optimization and cleanup path only. The lifecycle-gated
`publish_connected_provider_state` call remains mandatory and authoritative.

`ConnectionState::Connected::identity_generation` is descriptive captured
currentness evidence. `current_host_composition` compares it to the current
identity generation and rejects stale composition. Effective Authority also
uses the same currentness result, so changing identity while Connected makes
the old composition reconnect-required; it does not hot-recompose the
runtime. The old Commit capability and workflow are withdrawn by the identity
writer.

## 5. Identity-wins and Connect-wins races

The source ordering is conformant for the identity race: Connect prepares one
captured tuple and capability, `set_commit_identity` serializes through the
lifecycle gate and increments identity generation while withdrawing old
Commit state, and the final Connect gate rejects the old tuple. Current
identity remains the new identity and the rejected runtime/provider owner is
shut down.

The deterministic test
`task_321_e_identity_change_at_connect_publication_barrier_is_stale` and the
five-field currentness unit test pass. The barrier test directly proves the
identity mismatch at the intended barrier, but it is a narrower helper-level
test than the requested full pending-registry/capability/publication sequence;
the source audit supplies the remaining publication proof.

For Connect-wins, publication is lifecycle serialized first. A subsequent
identity write revokes the old pending Commit authorization and workflow, and
the Connected composition becomes stale/reconnect-required. No hot runtime
recomposition is claimed.

## 6. Index reservation design and linearization

`RepositoryIndexEffectReservation` at `main.rs:1306-1313` contains a checked
process-local token, repository generation, active member binding, Stage or
Unstage kind, and the exact `Arc<DesktopRepository>`. It is private, not
serializable, not persistent, not model/provider-visible, and has no
user-controlled token. The token is monotonic and allocated with
`checked_add`; overflow returns `RepositoryBusy` before reservation or action
consumption.

`begin_repository_index_effect` takes lifecycle coordination and checks that
no reservation exists, a repository and active member exist, the action kind,
repository generation, observation generation, member, generation, and exact
repository Arc are current, and pending Commit revocation can be acquired
synchronously. A busy Commit slot returns bounded `RepositoryBusy` before
consuming the action or installing a reservation.

The attempt linearization point is after those checks and successful Commit
revocation, but before the asynchronous target check and Tool call: the
selected action catalog is consumed, review/Commit review is cleared, and the
reservation is published. A later target failure is therefore a reserved
single attempt and performs cleanup/refresh; a pre-reservation failure leaves
the action available.

The lifecycle guard is released before target validation, Git Tool execution,
or refresh. The Tool is constructed from the captured repository Arc. No
blocking lifecycle guard crosses an await.

## 7. Stage race

`task_321_e_stage_reservation_wins_over_activation` reaches a barrier after
the reservation is installed and before the underlying Stage Tool call. B
activation returns `RepositoryBusy`, A remains active, and the repository
generation is unchanged. After the single Stage path and captured-A refresh,
the reservation clears and fresh B activation succeeds.

The test passed. The underlying `rah-tools` Stage tests retain nested-boundary,
identity, one-attempt, no-replay, and uncertain-result protections. The
Desktop barrier test does not independently instrument a native attempt
counter, so the one-attempt claim is the combined source and Tool-test result,
not a new Desktop counter assertion.

## 8. Unstage race

`task_321_e_unstage_reservation_wins_over_activation` has the same barrier
placement and ordering for Unstage. The reservation is observable before the
activation attempt, B is rejected busy, A remains active, the Unstage path
finishes once, cleanup occurs, and fresh B activation succeeds. The test
passed, with the same combined Desktop/source and `rah-tools` attempt evidence
qualification as Stage.

The existing stale-selector test also rejects an old A action after B is
active without mutating B. No test or source path permits an A action to be
resolved against B, and nested repository target validation remains in the
underlying Stage/Unstage tools.

## 9. Activation conflict

`publish_activation_if_current` acquires membership coordination and lifecycle
coordination, checks the reservation, and returns `RepositoryBusy` before
clearing prepared HostExplicit state, Commit/workflow state, conversation
namespace, or active repository state. It does not clear the reservation or
cancel the index Tool. This preserves A and its generation on the failed
switch.

The reverse ordering is also fail-closed in source: if activation publishes B
first, it invalidates A's workflow/action catalog and increments repository
generation; a later A action fails before reservation and therefore makes zero
Stage/Unstage attempts. The existing selector and Task 320 tests cover the
state-preservation portion. There is no new barrier test that counts the zero
attempts at this exact activation-wins boundary.

## 10. Disconnect, model, reset, identity, authorization, and Connect conflicts

Disconnect, `set_model_configuration`, `reset_model_preferences`,
`set_commit_identity`, and reviewed Commit authorization all check for an
active reservation after taking lifecycle coordination. They return bounded
busy before changing connection state, preferences, generations, Commit
revocation, workflow, or authorization. A candidate prepared before a
reservation is invalidated through the cleared workflow/review state before
the final authorization installation gate; no `AuthorizedPending` candidate
can be published.

There is, however, a material missing barrier for Connect. The initial
`connect_codex` lifecycle block at `main.rs:7211-7242` checks only host
invocation state before changing `NotConnected` to `Connecting`; it does not
check `repository_index_effect_reservation`. The authoritative final
`publish_connected_provider_state` gate at `main.rs:2014-2116` checks
connection state and the five generation fields, but also does not check the
index reservation.

The reachable ordering is:

```text
A is active and disconnected; an A action is observed
-> begin_repository_index_effect installs A's reservation and pauses before Tool execution
-> connect_codex starts Connecting because no reservation check exists
-> runtime/registry/provider composition is prepared
-> publish_connected_provider_state publishes Connected while A's effect is unresolved
-> A Stage/Unstage continues and refreshes afterward
```

This violates the required conservative rule that Connect must be rejected or
its final publication blocked while an index reservation remains active. It
allows a runtime composition to become current-looking during an unresolved
host index authority effect. The smallest correction is a reservation check
under lifecycle coordination at Connect admission and, independently, at the
authoritative final publication gate; stale Connect cleanup must leave no
Connected state or provider owner. This is an implementation correction within
ADR 0027 and does not require an ADR change.

## 11. Post-effect refresh audit

`refresh_repository_workflow_for_index_effect` validates token, kind, and the
current reservation binding before the asynchronous refresh and again after
it. The binding helper checks active member, repository generation, and exact
repository Arc. `complete_repository_index_effect` keeps the reservation
through refresh and only then clears it under lifecycle coordination, so an
ordinary activation cannot publish B while captured-A workflow publication is
pending.

`refresh_repository_workflow` captures repository and generation before
observation and rejects a changed repository generation before workflow
installation. During a valid reservation, all official active-repository
transition writers are lifecycle-blocked. The remaining Connect omission in
section 10 is why the overall lifecycle audit cannot close: Connect is a
competing publication path not covered by the reservation barrier.

Known Tool failure and uncertain Tool output are not retried or replayed. The
single terminal result still goes through refresh and reservation cleanup;
uncertainty is not converted into known no-effect. Crash and process-restart
recovery are not claimed, and the process-local reservation is not restored.

## 12. Stale-token cleanup

The stale-completion test installs token 2 before token 1 completion attempts
cleanup. Token 1 returns `RepositoryBusy` and token 2 remains installed; this
is not a trivial `None` proof. Normal allocation is checked and monotonic, so
an old completion cannot accidentally clear a later reservation. The test
passed.

## 13. Lock-order and deadlock audit

The reviewed reservation paths use lifecycle coordination before reservation,
repository/member/generation/workflow state, Commit revocation, token
allocation, and final reservation publication. Activation uses membership
coordination before lifecycle coordination, then validates state and performs
the synchronous commit point. Disconnect and the model/reset/identity writers
take lifecycle coordination before their state locks. Connect publication uses
lifecycle coordination before connection/currentness/provider publication.

Reservation completion takes lifecycle coordination for each short validation
or cleanup section and releases it before observation or Tool awaits. No
`std::sync::MutexGuard` crosses an await, and no lifecycle path waits
asynchronously while holding the lifecycle guard. I found no additional
material reverse acquisition or deadlock in the reviewed Task 321-C/E paths.

## 14. Active-only authority

The reservation is held only for the current active repository operation. An
inactive membership record contains no repository Arc, reservation, Stage or
Unstage Tool, workflow, Commit capability, ToolRegistry, provider activation,
or runtime. Switching remains a fresh active-only composition boundary.

## 15. Public-contract diff

The Task 321-D to Task 321-E production change is Desktop-only. There is no
Tool name/schema change, PermissionLevel change, provider protocol change,
Trusted Profile schema change, Tauri IPC change, frontend change, Cargo
dependency, ADR, or HostExplicit eligibility change. The exact HostExplicit
set remains:

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

`repo.commit` remains HostExplicit-ineligible, and Stage/Unstage remain
separate host index authorities.

## 16. Deterministic validation

All commands below were run at the pinned final head:

- `cargo fmt --check` - PASS.
- `cargo check --workspace` - PASS.
- `cargo test --workspace -- --test-threads=1` - PASS. Desktop: 238 passed,
  10 ignored; all workspace packages and integration suites passed. The
  ignored tests are host/live probes and do not constitute live certification.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` -
  PASS.
- `git diff --check` - PASS before this audit document was created; it is
  rerun after the document is written.
- `cargo metadata --no-deps --format-version 1` - PASS.
- `cargo build -p rah-desktop --release` - PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` - PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js` - PASS.
- `node crates/rah-desktop/tauri_permission_test.js` - PASS.
- Focused `cargo test -p rah-desktop task_321_e -- --nocapture
  --test-threads=1` - 4 passed, 0 failed, 244 filtered out.

The full run retained Task 321-C authorization, Task 320 activation atomicity,
Task 318 active-only switching, Task 316 admission, Task 315 nested-boundary,
repository Commit, Stage/Unstage, and HostExplicit regressions. No live v0.26
certification was run or inferred.

The v0.25.0 tag query returned annotated object
`ea3c31aaf5190b632d7ef86387f7aff6004ae664` peeling to
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` locally and from `origin`; the
GitHub Release query returned database ID `387406579` for tag `v0.25.0`.

## 17. Limitations and nonclaims

This audit does not claim v0.26 live certification, race-free TOCTOU,
rollback, replay safety after crash, OS sandboxing, network isolation, or
cross-platform live parity. It does not add persistence, member removal,
connected runtime migration, or release work. The identity barrier test and
Stage/Unstage activation tests provide deterministic coverage of their tested
boundaries, but do not replace a full runtime/provider Connect-vs-reservation
barrier test or a native-attempt counter at the activation-wins boundary.

## 18. Exact next task

Task 322 - Multi-Repository Remaining Product Scope Decision remains blocked
until the Connect-vs-index-reservation correction is implemented and receives
an independent re-audit. After a conformant re-audit, Task 322 remains
research/decision only and must compare the stated options A-D; no option is
implemented automatically.

## 19. Final verdict

F-321-F-1 is a material lifecycle/currentness defect: Connect can publish
while an active Stage/Unstage reservation remains unresolved. The smallest
correction is the final Connect barrier and corresponding deterministic test;
no ADR impact is indicated.

Verdict B — CORRECTION STILL REQUIRED
