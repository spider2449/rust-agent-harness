# Task 405 — Effective Authority Observation / Expiry Contract Clarification

Date: 2026-09-24
Scope: Documentation and contract research only.

## Verdict

**PASS — EFFECTIVE AUTHORITY CONTRACT CLARIFIED WITH TEST FOLLOW-UP RECOMMENDED**

Documentation now distinguishes authority decisions, snapshot observation, and
expired HostExplicit workflow bookkeeping. Existing tests support the expiry
and presentation rules separately, but a focused production snapshot-helper
test would strengthen evidence that gathering itself only clears an expired
preparation and never creates a ticket.

## Starting checkpoint

- Expected and verified HEAD: `c00fddd33fd80cb09a2cddc0dae01d26b98e6304`
  (`refactor: separate effective authority snapshot composition`).
- Expected and verified worktree: clean; `git status --short` was empty.
- Source inspected at this checkpoint. No Rust, tests, policy, expiry behavior,
  IPC/schema, permissions, capabilities, or dependencies were changed.

## Source behavior verification

The production call is in
`crates/rah-desktop/src/main.rs::effective_authority_snapshot_for_state`, at
`main.rs:2427` in the starting source. After cloning published Tools, root
locks `state.host_invocation`, calls
`coordinator.reap_expired(std::time::Instant::now())`, immediately copies
`coordinator.state()`, then drops the lock. The copied `CoordinatorState` is
passed as closed input to `effective_authority::compose_effective_authority_snapshot`.

`crates/rah-desktop/src/host_invocation.rs::PreparedHostInvocation::is_expired`
returns true when `now.duration_since(created_at) >= BRANCH_TICKET_TTL`.
`HostInvocationCoordinator::reap_expired` acts only when coordinator state is
`HostPrepared` and the retained prepared ticket satisfies that predicate. It
calls `clear_prepared`, which clears `prepared`, sets `preparing` false, and
sets state to `Idle`.

The ticket is unusable because it has already reached its expiry boundary,
before snapshot cleanup runs. Independently, `take_prepared` removes the
retained ticket and rejects it when `ticket.is_expired(now)` is true. Thus
reaping does not cause a still-valid ticket to become invalid; time expiry
already did so. Cleanup does not issue or consume a ticket through an
authorization path; it drops retained prepared state.

The cleanup can change `CoordinatorState` from `HostPrepared` to `Idle`. The
closed HostExplicit descriptor mapping uses that state to report busy versus
available/eligible presentation, so a subsequent snapshot may report changed
availability. No generation counter is advanced by these coordinator methods.
The cleanup does not call or mutate repository, Trusted Profile/provider,
runtime, or Commit authorization owners. It writes no persistence and makes no
explicit external I/O call. The coordinator code contains no route to those
owners or stores.

## Authority and bookkeeping distinction

1. **Executable authority decision:** grant/permission, preparation of a
   usable ticket, dispatch authorization, repository activation, provider or
   runtime authority change, or Commit authorization. Snapshot collection
   performs none of these and its output is not an execution token.
2. **Authority observation:** root gathers currentness and availability facts;
   deterministic composition reports Tools, HostExplicit availability,
   reviewed Commit presentation, and the closed snapshot.
3. **Expired workflow bookkeeping:** root may discard retained state for an
   already-expired, already-unusable prepared HostExplicit ticket before
   reporting coordinator availability. This can normalize busy/availability
   presentation without granting authority or invalidating a usable ticket.

Therefore `reap_expired` does not reduce or increase currently usable
executable authority. It clears the representation and retained resources of
a preparation that has already become unusable by elapsed time. The source
proves that state transition and independent ticket rejection. It does not
prove stronger guarantees about destruction side effects of arbitrary future
payload types; the current operation itself invokes no external I/O or
persistence API.

## Documentation inventory and classification

The source survey covered `README.md`, `docs/ARCHITECTURE.md`,
`docs/SECURITY.md`, ADRs 0011 and 0021 (plus the other ADR index/search hits),
and the Task 401–404 plans. No release/security gate document was found to
define a stronger normative Effective Authority contract.

| Existing statement | Classification | Reason |
|---|---|---|
| `ARCHITECTURE.md` described the path as a read-only Tauri command and said inspection does not mutate authority. | ACCURATE BUT AMBIGUOUS | Accurate about authorization decisions and execution; can be read as forbidding the root's existing expired-state cleanup. The command is now called an authority-observation command, and the contract scopes observation to authority decisions/executable capability while identifying cleanup. |
| `SECURITY.md` said inspection and refresh have “zero lifecycle, Tool, repository, chat, authority, or persistence side effects.” | TOO BROAD; the zero-lifecycle clause is CONTRADICTED BY CURRENT IMPLEMENTATION | Reaping mutates the coordinator's retained workflow state. It does not mutate executable authority, Tools, repository, chat, or persistence. Replaced the unqualified zero-side-effect claim with precise boundaries. |
| `README.md` calls Refresh Authority “read-only” and says the panel does not compose authority. | ACCURATE BUT AMBIGUOUS | “Read-only” may imply no workflow-state mutation. Updated to state observational scope and bounded cleanup explicitly. |
| ADR 0021 says Effective Authority remains observational and the snapshot is never dispatch authority. | ACCURATE BUT AMBIGUOUS | Correctly excludes dispatch authority, but does not define the root gathering cleanup. No decision conflict; broader docs now give the operational qualification. |
| ADR 0011 says effective authority is inspectable without becoming model-visible topology. | ACCURATE | Describes visibility/model boundary, not mutation purity. |
| Task 401 identifies expiry cleanup as a narrow wording discrepancy; Task 402 keeps it in root and calls documentation a separate follow-up. | ACCURATE | Research/design constraints match the checkpoint. |
| Task 403 records root gathering/cleanup and pure module composition; Task 404 confirms exact call position, closed input, and absence of ticket operations in composition. | ACCURATE | Independently audited implementation/topology facts; these task records are not broad zero-mutation guarantees. |

Other surveyed wording that display, visibility, configured intent, human
confirmation, or Tool advertisement is not authorization is accurate and
unchanged. Existing gated/release records were treated as historical evidence,
not retroactively edited. In particular, `docs/RAH_V0.16_RELEASE_GATE.md`
calls Refresh Authority read-only and says it has no lifecycle side effects;
the zero-lifecycle claim is too broad in light of the current cleanup call.
This gate records the historical v0.16 certification and was not rewritten.
`docs/RAH_V0.20_RELEASE_GATE.md` says Effective Authority is observational and
the frontend does not authorize or infer eligibility; that statement is
accurate but does not specify root-side workflow cleanup.

## Canonical observational contract

> Effective Authority snapshot collection is observational with respect to
> authority decisions and executable capability: it does not grant, issue,
> consume, or authorize executable authority. Root gathering may reap an
> already-expired, already-unusable HostExplicit preparation before reporting
> current availability. This bounded workflow-bookkeeping cleanup may change
> busy/availability presentation, but does not make a valid ticket unusable or
> change other authority owners. The `effective_authority` composition step
> receives closed facts and deterministically derives the snapshot; it performs
> no expiry cleanup or live-state access.

Each clause is supported by the inspected caller, coordinator, ticket validity,
and composition code described above.

## Architecture wording decision

`docs/ARCHITECTURE.md` now explicitly separates root gathering of live facts
and existing expiry bookkeeping from deterministic `effective_authority`
composition. The module is not described as owning `reap_expired`; its input
remains a copied `CoordinatorState`. HostExplicit remains exactly 11 and all
eligibility predicates/owners remain unchanged.

## Security wording decision

`docs/SECURITY.md` now says snapshot output is descriptive and never an
execution token; snapshot collection cannot grant, issue, consume, or authorize
executable authority; expired preparation cleanup is bookkeeping rather than a
grant path; and permission, review, and ticket checks remain mandatory. The
wording preserves authority-owner boundaries and does not imply new ticket,
permission, or policy behavior.

## README wording decision

`README.md` contained the relevant unqualified “read-only” description of
Refresh Authority, so it was narrowed. It now states the authority-observation
scope and acknowledges the bounded expiry cleanup and availability-presentation
effect.

## ADR assessment

This is clarification of behavior already present and deliberately preserved
through Tasks 401–404, not a new architectural policy. ADR 0011's inspectability
boundary and ADR 0021's non-dispatch snapshot boundary remain consistent. No
new ADR is required, and no existing ADR was edited.

## Test evidence

Relevant existing tests include:

- `ticket_expiry_is_deterministic_without_sleeping` — proves the TTL boundary.
- `ticket_matrix_is_single_use_for_valid_expired_cancelled_and_stale_paths` —
  proves expired tickets are rejected and expired coordinator state is idle
  after explicit reaping.
- `snapshot_composition_keeps_host_explicit_availability_presentation` —
  proves `HostPrepared` input maps to busy/unavailable presentation, while idle
  input maps to eligible presentation.
- `snapshot_composition_keeps_the_eleven_host_explicit_kinds` — proves the
  fixed set of 11 kinds.
- `composed_snapshot_keeps_closed_schema_and_safe_presentation` and
  `complete_snapshot_serialization_is_sanitized_and_closed` — prove closed,
  sanitized snapshot output.
- `branch_effective_authority_is_host_classified_and_unavailable_paths_are_closed`
  — supports host classification and unavailable presentation.
- Task 404's audit confirms `effective_authority::compose_effective_authority_snapshot`
  has no ticket operation, live owner access, or coordinator mutation.

The tests do not directly set up an expired production ticket, call the root
snapshot helper, and assert the precise coordinator transition, nor do they
directly prove absence of ticket issuance across that helper. Source inspection
supports both claims, but a focused deterministic test would make the contract
more durable. Test evidence is sufficient for this documentation clarification
with a future test-contract follow-up recommended; no test changes were in
scope.

## Documentation files changed

- `docs/ARCHITECTURE.md`
- `docs/SECURITY.md`
- `README.md`
- `docs/plans/2026-09-24-task-405-effective-authority-observation-expiry-contract-clarification.md`

No Rust or test files changed. No behavior, authority, expiry, HostExplicit
count, IPC/schema, permission/capability, or dependency changes were made.

## Validation and closeout

- Required validation: `git diff --check` — PASS; only LF-to-CRLF working-copy
  warnings for the three existing Markdown files were emitted.
- Rust/workspace/live validation: not run, as required for documentation-only
  work.
- Commit message: `docs: clarify effective authority observation contract`;
  the resulting commit SHA is recorded in the Task 405 closeout report.
- Push/tag: not authorized and not performed.
- Verdict: **PASS — EFFECTIVE AUTHORITY CONTRACT CLARIFIED WITH TEST FOLLOW-UP RECOMMENDED**.
- Recommended next task: **Task 406 — Effective Authority Snapshot Expiry
  Bookkeeping Contract Test**, limited to deterministic coverage of root
  snapshot gathering with an expired prepared ticket and the absence of ticket
  issuance. Do not start it automatically.
