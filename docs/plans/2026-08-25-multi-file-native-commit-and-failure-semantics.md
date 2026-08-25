# Task 094B — Multi-file native commit and failure semantics

Status: Complete private implementation; ADR 0014 remains Proposed.

## Native architecture

`repository_multi_file_preflight.rs` now contains the crate-private commit
engine beside the 094A retained-plan seam. It consumes exactly the prepared
postimages and does not reparse or recompute request semantics. The existing
repository mutation lease is acquired before preparation and remains held
through every revalidation, native replacement, outcome classification, and
bounded temporary cleanup.

Targets are committed in the already-prepared ascending UTF-8 byte lexical
order of canonical logical paths. Each target has exactly one native commit
point: the private `replace_once` invocation. Windows uses the certified
`MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` primitive; Unix uses a
same-directory `rename`. Neither primitive creates a cross-file transaction
or a crash-durability claim. Unix prepared temporary modes and final modes are
verified.

## Failure semantics

The crate-private result is `MultiFileEditOutcome` with `Ok`,
`InvalidTarget`, `PreconditionFailed`, `FailedKnownNoEffect`, `PartialEffect`,
and `Uncertain`, plus ordered redaction-safe logical-path effects:
`CommittedVerified`, `UnchangedVerified`, `NotAttempted`, and `Uncertain`.

Before each target, the engine rechecks repository identity, raw index, HEAD,
refs, parents, committed prefix postimages, uncommitted suffix preimages, Git
target state, and the owned temporary. Success is accepted only after target,
repository, index, HEAD, and ref observation. A known native no-effect
failure is classified only after the original state and the complete inventory
are proven. A verified committed prefix followed by such a failure is
`PartialEffect`; any ambiguous native outcome or lost post-commit
certification is `Uncertain` and stops immediately.

There is no retry, replay, rollback, Git mutation, recovery journal, or
continuation after uncertainty. Uncommitted temporaries are removed only after
identity/content proof; an unproven cleanup becomes uncertain and leaves
evidence intact.

## Deterministic evidence

Test-only hooks support before-call stop, known no-effect native failure,
uncertain native outcome, post-native lost certification, and
post-certification stop. Test-only per-target attempt counters prove zero or
one attempt and no later attempts after a terminal result. Focused tests cover
one/two/four target success, host ordering independent of request order,
first/middle/final known failures, verified partial inventory, first/middle/
final uncertainty, lost certification, no retry, no rollback, and preservation
of raw index, HEAD, and refs. Windows runs the actual `MoveFileExW` layer;
Unix CI exercises the rename path with self-contained fixture Git identity.

## Deferred to 094C

No `Tool`, ToolRegistry registration, Trusted Profile capability, Generic Tool
Bridge branch, PermissionLevel/profile-version change, or live Codex gate was
added. Task 094C remains responsible for the implementation-versus-contract
audit and any ADR 0014 acceptance decision.
