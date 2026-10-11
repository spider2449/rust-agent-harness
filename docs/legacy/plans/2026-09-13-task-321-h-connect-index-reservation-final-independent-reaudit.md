# Task 321-H - RAH v0.26 Connect / Index Reservation Final Independent Re-Audit

## 1. Authoritative checkpoint

| Item | Value |
| --- | --- |
| Audited master | `18f69a4123f27ef824674d916cb409a6b3f543e2` |
| Direct parent | `9d0f374ed7a6838f8177d4cf4ae32423338c3d00` |
| Task 321-G production commit | `9d0f374ed7a6838f8177d4cf4ae32423338c3d00` |
| Task 321-G documentation closure | `18f69a4123f27ef824674d916cb409a6b3f543e2` |
| Task 321-G exact-head CI | `34754317809` - PASS |
| Task 321-F outcome | Verdict B - CORRECTION STILL REQUIRED |
| Accepted architecture | ADR 0027 - Workspace/Repository Identity and Authority-Composition Boundary |
| Workspace | 13 packages, version `0.25.0`, Rust edition 2024 |
| HostExplicit eligible set | Exactly 11 names |

This is an audit-only document. No production code, test, frontend,
permission, Cargo, dependency, ADR, persistence, removal, release, or live
certification change was made by Task 321-H.

## 2. Task 321-F residual finding

Task 321-F identified a reachable ordering in which an active Stage/Unstage
reservation could coexist with Connect publication:

```text
A active and disconnected
-> index reservation installed and effect unresolved
-> Connect entered Connecting without checking the reservation
-> runtime/composition/provider/Commit state prepared
-> final publication omitted the reservation check
-> Connected state was published while A's effect continued
```

That was a material lifecycle/currentness defect. The current source removes
the production path through two independent reservation gates. The remaining
321-H finding is evidence-related and is recorded in section 17: the new
barrier tests do not exercise the real final publication or reservation-start
functions required by this audit.

## 3. Task 321-G change map

The cumulative diff from Task 321-F head
`85ffefb4d7b0dec1e3a6746580a67591b4567eeb` to the audited head contains the
expected two paths only:

```text
M  crates/rah-desktop/src/main.rs
A  docs/plans/2026-09-13-task-321-g-connect-index-reservation-correction.md
```

Task 321-G added `ProviderPublicationRejectionReason::IndexEffectActive`, the
admission helper `begin_connect`, the final publication reservation gate,
bounded rejection mapping, and three deterministic test functions. No
`rah-tools` production path, Tool name/schema, PermissionLevel, provider
protocol, Trusted Profile schema, frontend, Tauri permission, Cargo/dependency,
ADR, or HostExplicit eligibility change is present.

## 4. Connect admission audit

`begin_connect` at `crates/rah-desktop/src/main.rs:7231-7257` acquires
`lifecycle_coordination`, checks `HostInvocation` state, locks connection
state, checks the reservation for `NotConnected` or `Error`, and only then
calls `request_connect`. An active reservation returns the bounded
`FrontendError::RepositoryBusy` before the `Connecting` transition.

The rejected path therefore leaves the connection `NotConnected` or the
existing `Error`, does not advance `next_connection_generation`, and does not
reach runtime construction, first-party registry construction, provider
activation, or Commit capability construction. The current source also keeps
the reservation token, repository binding, member binding, and action kind
untouched. The check is locked; it is not an unlocked preliminary check.

The early-admission implementation is conformant. The required test evidence
is not fully conformant: `task_321_g_connect_admission_rejects_stage_and_unstage_reservations`
uses `install_test_index_effect_reservation`, which writes the private
reservation field directly, rather than installing it through
`begin_repository_index_effect`.

## 5. Connect final publication audit

`publish_connected_provider_state` at
`crates/rah-desktop/src/main.rs:2015-2142` acquires
`lifecycle_coordination` before connection inspection. It requires
`ConnectionState::Connecting`, then calls
`connected_publication_index_effect_rejection` at lines 2039-2049. Only after
that check does it read and compare the five currentness fields, check the
provider owner, and destructure the pending runtime, provider activation,
composition, permissions, and Commit capability for publication.

The reservation check is independent of admission and is structurally before
provider owner publication, Connected state, composition, and Commit
capability publication. The same lifecycle guard remains held through the
final assignment at lines 2130-2141. A reservation cannot be inserted between
the check and publication in production code.

The source ordering is conformant. The required final-gate test is not: the
Stage test pauses `connect_pre_publication_barrier`, directly installs a
reservation, and calls `connected_publication_index_effect_rejection` itself. It
does not call `publish_connected_provider_state`. The Unstage test only calls
the same helper directly and has no real publication barrier. Thus these tests
do not independently prove that the real publication function observes the
reservation under its lifecycle guard.

## 6. Race linearization proof

The production functions use the same exclusion:

* `begin_repository_index_effect` acquires `lifecycle_coordination` before
  checking and publishing the reservation (`main.rs:6198-6289`).
* `publish_connected_provider_state` acquires it before checking the
  reservation and holds it through publication (`main.rs:2019-2141`).

The source therefore has the two safe orderings:

```text
Order A - reservation wins:
  reservation exists before admission -> RepositoryBusy
  or reservation starts before final publication -> IndexEffectActive rejection

Order B - Connect publication wins:
  final publication owns lifecycle exclusion first -> Connected publishes
  Stage/Unstage can begin only after that publication according to current policy
```

No Connected publication occurs while the reservation remains active in Order
A. In Order B, activation remains blocked by Connected state, and once the
reservation begins, Disconnect, model/reset, identity, and reviewed Commit
authorization remain lifecycle-blocked. The captured repository Arc and
generation remain the effect binding. This is a source proof; the required
real-path race test is missing as described above.

## 7. Runtime, provider, Commit, and composition cleanup

`connect_codex` passes pending runtime and provider activation into
`publish_connected_provider_state` at `main.rs:7573`. On rejection it owns
the returned `RejectedProviderPublication`, attempts bounded runtime shutdown,
then shuts down returned provider activation before mapping
`IndexEffectActive` to `RepositoryBusy` (`main.rs:7574-7605`). No rejected
runtime is stored in `ConnectionState`, `provider_activation`, or another
side channel. No retry or replay is introduced.

The pending Commit capability is not part of `RejectedProviderPublication`;
it is dropped with the unpublished pending structure. The publication function
does not temporarily install it. Pending composition and permissions are
likewise dropped without becoming `current_host_composition` or Effective
Authority. A later Connect constructs a fresh pending runtime, registry,
provider composition, and capability.

These ownership conclusions are based on the actual transfer and drop paths,
not comments. A direct runtime/provider cleanup test for this rejection reason
is not present; the source ownership is clear, while live provider effects are
outside this deterministic audit.

## 8. Rejection reason and connection state

`ProviderPublicationRejectionReason::IndexEffectActive` is a private enum
variant. It is not serialized into provider or model data. The only frontend
mapping is the bounded `FrontendError::RepositoryBusy`; no token, path, Arc
identity, generation tuple, or provider ownership is returned.

After cleanup, the rejection branch changes `Connecting` to `NotConnected` and
returns `RepositoryBusy`. If another transition has already replaced the
connection state, the conditional assignment preserves that newer state. No
stuck `Connecting`, false `Connected`, stale Error authority, or automatic
retry is introduced.

## 9. Reservation preservation and effect completion

The Connect rejection does not clear, rotate, cancel, or reinterpret the
reservation. The original Stage/Unstage owner retains the token, kind,
repository Arc, member, and generation binding. `complete_repository_index_effect`
keeps the reservation through the Tool terminal result, captured-repository
refresh, final binding validation, and cleanup, then clears only the matching
token. Stale completion cannot clear a newer token.

The underlying Stage and Unstage implementations remain one-attempt paths.
Known failure and uncertainty do not cause Connect-driven replay or retry. A
fresh Connect becomes admissible only after normal reservation completion.

## 10. Stage admission and final-gate evidence

The Stage portion of
`task_321_g_connect_admission_rejects_stage_and_unstage_reservations` passed
with `RepositoryBusy`, `NotConnected`, unchanged connection generation, zero
startup activation counters, no provider owner, no Commit capability, and
preserved token/kind.

The Stage final-gate test passed its helper-level assertions and preserved the
reservation while paused. It did not call the real publication function, so
it is not sufficient evidence for the requested production-path barrier.

## 11. Unstage admission and final-gate evidence

The Unstage half of the shared admission test independently passed the same
state and startup assertions, preserving the Unstage kind. This independently
covers action-kind-independent admission wiring.

`task_321_g_unstage_reservation_is_seen_by_final_publication_gate` passed only
the direct helper classification. It does not pause a real Connect
publication, call `publish_connected_provider_state`, or prove Unstage
reservation installation through `begin_repository_index_effect`.

## 12. Connect-first and Connect-wins ordering

The real Connect path places `connect_pre_publication_barrier` immediately
before currentness reads and the call to `publish_connected_provider_state`
(`main.rs:7491-7573`). This is the intended barrier placement. The present
tests invoke that hook in isolation rather than driving `connect_codex` through
runtime preparation and final publication; this is the central audit finding.

The opposite ordering remains coherent in source. The current product permits
Stage/Unstage under its existing connected-state policy. Once publication wins,
activation cannot migrate the repository because Connected state blocks it;
when Stage/Unstage subsequently reserves the index lifecycle epoch,
Disconnect, model/reset, identity, and Commit authorization writers reject
busy. No operation migrates the captured Arc or refreshes another repository.

## 13. Task 321-E, 321-C, 320, 317, 318, and 315 regressions

* Task 321-E reservation fundamentals remain present: checked monotonic token,
  exact repository Arc, repository generation, active-member binding, action
  kind, one active reservation, Commit revocation before reservation
  publication, no await under the lifecycle guard, activation and writer
  blocking, stale-token protection, and captured-repository refresh checks.
* Task 321-E Connect currentness retains repository, model, profile,
  connection, and Commit identity generations in
  `ConnectionPublicationCurrentness` and compares all five at final
  publication.
* Task 321-C retains one non-authoritative prepared candidate, exact control
  binding, currentness checks, one shared pending slot, one-shot install,
  stale re-arm rejection, and lifecycle serialization. The focused
  authorization test passed.
* Task 320's three focused atomicity tests passed: stale target preserves the
  active state, the loser preserves the winner, and successful switching
  invalidates old Commit/workflow state coherently.
* The activation-family focused filter passed 14 tests, including the Task
  317 connection/model/HostInvocation/prepared-confirm/concurrent-activation
  cases and retained Task 321-C/E cases. The full Desktop suite also passed.
* Task 318's active-only selector/switch test passed. Inactive members remain
  descriptive and non-executable; no union registry or model-selected
  repository route exists.
* Task 315's real nested-boundary integration test passed, as did the
  repository nested-boundary and capability regressions in the full workspace
  run. Membership/reservation does not replace path isolation.

## 14. Lock order and deadlock audit

Connect admission, Connect final publication, and reservation installation
acquire `lifecycle_coordination` before their competing state access. The
reviewed repository writers use the same lifecycle-first discipline. The
reservation lock is not acquired first on a competing production path. No
blocking lifecycle guard crosses an async runtime/provider shutdown, Tool
execution, refresh, or other await; shutdown occurs after publication locks
are released.

The existing activation path's membership coordination precedes lifecycle
coordination as documented by Task 317; no reverse cycle involving the
reservation path was found. No material deadlock was found. The direct test
fixture's field assignment is not production synchronization and is included
in the evidence limitation, not treated as a production lock-order proof.

## 15. Active-only authority, public contract, privacy, persistence

Inactive members retain no executable repository Arc, index reservation,
workflow, DesktopRepository, ToolRegistry, Commit capability, runtime,
provider owner, or HostExplicit state. There is no union executable state.

The exact 11 HostExplicit names remain:

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

`repo.commit` remains HostExplicit-ineligible. Stage and Unstage remain
separate host index authorities. No Tool/schema, permission, provider,
Trusted Profile, frontend, Tauri, Cargo, dependency, or ADR drift was found.

Reservation state and `IndexEffectActive` are private. Frontend output is only
`RepositoryBusy`; token, Arc identity, native path, generation tuple, and
provider ownership do not enter public or model-visible data. Workspace
membership persistence, reservation persistence, active-member restore, member
removal UI/IPC, and pending connection authority persistence remain absent.
Restart restores none of this authority.

## 16. Deterministic validation and immutable release identity

All requested deterministic checks passed at the audited head unless noted as
the evidence limitation in section 17:

```text
cargo fmt --check                                      PASS
cargo check --workspace                                PASS
cargo test --workspace -- --test-threads=1             PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS
git diff --check                                       PASS
cargo metadata --no-deps --format-version 1           PASS
cargo build -p rah-desktop --release                   PASS
```

The full serialized workspace test counted Desktop 241 passed / 10 ignored,
`rah-runtime-codex` 83 passed / 1 ignored, `rah-tools` 288 passed, and all
other workspace, integration, and doc-test suites passed with zero failures.
The focused results were:

```text
cargo test -p rah-desktop task_321 -- ...              8 passed
cargo test -p rah-desktop task_320 -- ...              3 passed
cargo test -p rah-desktop task_318 -- ...              1 passed
cargo test -p rah-desktop activation -- ...            14 passed
cargo test -p rah-tools repository_commit -- ...       18 passed
cargo test -p rah-tools --test repository_nested_boundary  1 passed
frontend status syntax/authority/membership tests      PASS
Tauri permission tests                                  PASS
```

The v0.25.0 immutable identity remains source
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, annotated tag object
`ea3c31aaf5190b632d7ef86387f7aff6004ae664`, and GitHub Release database ID
`387406579` for tag `v0.25.0`. The release is not the current v0.26 audit
head.

## 17. Limitations and material correction required

This audit is deterministic and static. It does not claim current-master
v0.26 live certification, live Codex/provider activation, native live effect
certification, race-free TOCTOU, rollback, crash recovery, OS sandboxing,
network isolation, or cross-platform live parity.

The material remaining issue is test-path assurance: the Task 321-G final-gate
tests directly call a classification helper and directly assign the private
reservation instead of exercising (a) `begin_repository_index_effect`, (b)
the actual Connect preparation path, and (c) `publish_connected_provider_state`
under the real lifecycle exclusion. Production source inspection proves the
intended ordering, but the task's required barrier test explicitly requires
the real publication function, so this audit cannot close that requirement.

Smallest correction: add deterministic Windows Desktop tests that install
Stage and Unstage through the real reservation-start path, pause actual Connect
at the pre-publication barrier, resume the actual
`publish_connected_provider_state`, and assert rejected runtime/provider/
Commit/composition ownership plus retryable `NotConnected` state. No ADR
change, public contract change, or production authority redesign is indicated.

Task 322 remains blocked. The next task is **Task 321-I - Real Connect/index
publication and reservation-installation barrier evidence correction**. It
must be correction/test evidence only and must not add persistence, removal,
release work, or live certification. If Task 321-I closes this evidence gap,
the subsequent Task 322 research/decision should start from Option A: stop
feature expansion and proceed toward milestone audit, Windows two-repository
live certification, and the v0.26 release path, unless concrete product
evidence justifies expanding the authority/lifecycle surface.

## 18. Final verdict

Verdict B — CORRECTION STILL REQUIRED
