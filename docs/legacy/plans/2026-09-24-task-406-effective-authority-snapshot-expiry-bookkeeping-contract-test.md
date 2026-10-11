# Task 406 — Effective Authority Snapshot Expiry Bookkeeping Contract Test

## Verdict

**PASS — EFFECTIVE AUTHORITY EXPIRY BOOKKEEPING CONTRACT TESTED**

## Starting checkpoint

- HEAD: `686a4b9820e317324d20a77fe83009ec701a9203` (`docs: clarify effective authority observation contract`).
- Worktree: clean; `git status --short` was empty.

## Task 405 contract

Effective Authority snapshot collection is observational with respect to
authority decisions and executable capability. It does not grant, issue,
consume, or authorize executable authority. Root gathering may reap an
already-expired, already-unusable HostExplicit preparation before reporting
current availability. This bounded workflow-bookkeeping cleanup can change
busy/availability presentation, but does not invalidate a valid ticket or
change other authority owners. `effective_authority` receives copied
`CoordinatorState` facts and deterministically derives presentation without
expiry cleanup or live-state access.

## Existing evidence and identified gap

Preflight inspected these existing tests:

- `host_invocation::tests::ticket_expiry_is_deterministic_without_sleeping`
  proves the TTL boundary directly.
- `host_invocation::tests::ticket_matrix_is_single_use_for_valid_expired_cancelled_and_stale_paths`
  proves coordinator expiration cleanup and single-use behavior.
- `tests::branch_effective_authority_is_host_classified_and_unavailable_paths_are_closed`
  proves host classification and closed unavailable paths.
- `effective_authority::tests::snapshot_composition_keeps_host_explicit_availability_presentation`
  proves presentation for copied Idle and HostPrepared states.

Together these did not directly prove that root snapshot gathering itself calls
`reap_expired` before reporting HostExplicit availability. Production source
inspection confirmed `effective_authority_snapshot_for_state` invokes
`coordinator.reap_expired(Instant::now())` immediately before observing
`coordinator.state()`.

## Preflight fixture/helper inventory

The smallest existing path that yields a current snapshot with actual
HostExplicit descriptors is in `crates/rah-desktop/src/main_tests.rs`:

- `activation_fixture()` supplies `DesktopAppState` with an active repository.
- `desktop_tool_registry` and `desktop_tool_composition_from_registry`
  construct the existing repository composition.
- `test_codex_runtime` provides the existing deterministic fake runtime.
- `begin_connect` and `publish_connected_provider_state` publish a current
  connection using `PendingConnectedPublication`.
- `PreparedHostInvocation::for_test` plus coordinator `prepare` create a real
  prepared HostExplicit coordinator state through existing test support.
- `effective_authority_snapshot_for_state` is the root gathering seam used by
  the Tauri command.

## Frozen test boundary

Exactly one new deterministic test was frozen before editing:

- Test: `effective_authority_snapshot_reaps_expired_preparation_without_issuing_authority`
- Location: `crates/rah-desktop/src/main_tests.rs:11918-12039`.
- Existing fixtures/helpers only; no production changes, existing test edits,
  new production API, visibility change, test-only hook, or broad matrix.

The ticket is created with `created_at = Instant::now() - BRANCH_TICKET_TTL`
and its `is_expired(Instant::now())` predicate is asserted before insertion.
This uses the production TTL and validity rule. No sleep, waiting, or wall-clock
delay is used.

## Contract evidence

Before snapshot gathering, the test asserts that the coordinator is
`CoordinatorState::HostPrepared` and independently asserts the prepared ticket
is already expired. It then invokes the actual root helper
`effective_authority_snapshot_for_state`.

After gathering, the test asserts:

- Snapshot status is `ConnectedCurrent`.
- Coordinator state is `Idle`.
- The `repo.status` HostExplicit descriptor is eligible and has no unavailable
  reason, reflecting the cleaned-up state.
- `take_prepared("test-ticket", ...)` returns `NotPrepared`; snapshot gathering
  did not leave a newly usable ticket.
- Repository and Trusted Profile generations, repository workflow Commit
  authorization, provider activation presence, Trusted Profile selection
  presence, and Commit capability presence are unchanged across gathering.
- No HostExplicit confirmation/dispatch function is called by the test or root
  snapshot path. The expired ticket cannot be taken, so no executable
  authorization is available to dispatch.
- Existing prepared-ticket cleanup remains owned by
  `HostInvocationCoordinator`; the snapshot gatherer only invokes its cleanup
  before copying state; `effective_authority` remains a presentation derivation
  over copied facts.

## Production and scope audit

- Production Rust changes: none.
- Visibility changes: none.
- Expiry rules, authority policy/composition, HostExplicit kinds,
  permissions/capabilities, dependencies, frontend, and test topology: none.
- Existing tests and ignored markers: unchanged.
- Live test identities: unchanged. No Windows live certification ran; this
  contract is established by deterministic package tests.
- Workspace validation: not run; the change is confined to one Desktop test
  source and this Task 406 plan, with no production API, dependency, or
  cross-crate changes.

## Test-count delta

- Baseline: 319 passed, 18 ignored, 337 discovered.
- Expected delta: one added deterministic test.
- Current: 320 passed, 18 ignored, 338 discovered, 0 failed.

## Focused validation

- `cargo fmt --check`: PASS.
- `cargo test -p rah-desktop -- --test-threads=1`: PASS — 320 passed, 18
  ignored, 0 failed, 338 discovered.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings`:
  PASS.
- `git diff --check`: PASS.
- The new test also passed in isolation: 1 passed, 0 failed.

## Static contract audit

| Question | Result |
| --- | --- |
| Was preparation already expired before snapshot gathering? | Yes; asserted with the production `is_expired` rule. |
| Did gathering invoke the actual root path? | Yes; `effective_authority_snapshot_for_state`. |
| Did cleanup happen through existing `reap_expired` behavior? | Yes; root gathering calls it before observing state. |
| Did reporting change only because coordinator state was normalized? | Yes; the closed descriptor reports availability from resulting `Idle` state. |
| Was any new ticket issued? | No; no preparation/issuance path is called and the ticket cannot be taken afterward. |
| Was any executable operation dispatched? | No; snapshot gathering has no dispatch call and no usable ticket remains. |
| Did any unrelated authority owner change? | No; the test compares repository/profile generations, workflow authorization, provider activation, Trusted Profile selection, and Commit capability presence. |
| Did any production source change? | No. |

## Changed files and closeout

Exact changed files:

- `crates/rah-desktop/src/main_tests.rs`
- `docs/plans/2026-09-24-task-406-effective-authority-snapshot-expiry-bookkeeping-contract-test.md`

Commit SHA/message, post-commit status, and final diff audit are recorded in the
Task 406 closeout report. No push or tag is authorized or performed.

Recommended next task: **Task 407 — Reassess Desktop Production Decomposition
After Effective Authority Seam** (research-only; do not begin automatically).
