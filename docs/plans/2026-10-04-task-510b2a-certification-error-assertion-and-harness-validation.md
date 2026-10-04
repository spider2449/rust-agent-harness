# Task 510B2A — Certification error assertion and harness validation

Starting and final HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserved dirty Task 510B/510B1/510B2 implementation and reports.

Plan: inspect the existing typed cause before correcting assertions; if it
retains exact verification evidence, correct only tests and run the original
focused command, feature graphs, installed controls/smoke, deterministic and
canonical Desktop gates sequentially. Stop if the typed evidence is absent.
Only classification A permits commit, push and exact-head CI.

## B — CERTIFICATION VERIFICATION ERROR LOSES ITS TYPED SOURCE

The Task 510B2 report records the previous focused run as 2 passed / 1 failed /
0 ignored / 103 filtered out, exit 1. Its failing assertion expected
`RuntimeFailure::to_string()` to contain `SHA-256 mismatch`. That assertion
violates Task 498: Display projects sanitized RuntimeDiagnostic; private
evidence must be inspected through Error::source and typed downcasting.

Source inspection establishes an additional blocker before test correction:

- `certification_support.rs:29-33`: rejection constructs only
  `CodexAdapterError::ProtocolViolation { message }`.
- `certification_support.rs:93-94`: hash mismatch replaces both expected and
  measured SHA with the constant `certification SHA-256 mismatch`.
- `certification_support.rs:108-113`: version mismatch replaces expected and
  reported version with the constant `certification reported version mismatch`.
- `certification_support.rs:39-42`: missing/relative-path rejection omits the
  supplied path and collapses distinct path failures into a constant message.
- `certification_support.rs:132-135` and `errors.rs:118-155`: conversion retains
  that adapter error as RuntimeFailure's source. `failure.rs:63-69` exposes it
  through Error::source. The neutral envelope does not discard its cause; the
  certification verifier discards exact evidence before constructing the cause.

Downcasting to CodexAdapterError is supported, but yields only a generic
ProtocolViolation containing a constant string. There is no structured exact
verification subtype or retained hash/version pair to assert. Parsing that
message or Debug would not meet the requested typed semantics. This satisfies
the explicit section 9 stop condition: the existing typed cause is insufficient.

No assertion correction was applied: changing it to check only the generic
variant would conceal the missing evidence. No focused rerun was performed;
there are no new executable test counts. Public Display remains unchanged and
sanitized by source inspection; exact private verification details are not
preserved. Full Task 498 verification compliance is therefore not established.

## Narrow correction recommendation

Preserve a certification-specific process-local typed verification cause with
distinct hash-mismatch, version-mismatch and path-failure variants. Retain exact
expected/actual hashes, expected/reported versions and relevant supplied path
in that cause, and propagate it through RuntimeFailure::new or an adapter source
wrapper without formatting it into the public diagnostic. Keep the existing
sanitized Display and neutral error API. Do not merely put values into strings.
This correction is recommended, not implemented under this stop instruction.

Then separately authorize re-entry to correct tests using source/downcast and
exact field assertions plus public redaction assertions, and run the original
focused command before any installed-artifact execution.

## Validation and nonclaims

Default/certification feature graphs, all-features runtime safety audit,
installed production rejection, installed wrong-hash/version controls,
exact-path/PATH control, real candidate smoke, adapter/neutral runtime/Desktop
composition, shutdown and pre/post artifact hashes: not executed in this task.
The historical Task 510B2 feature-graph evidence is not recertified here.

Workspace fmt/check/test/Clippy and canonical Windows Desktop suite: not run.
Workspace/Desktop counts and exit codes: unavailable for this task.
Tauri 47/47/47/47/47, metadata 14 packages at 0.33.0 and executable/static
HostExplicit 11: not rerun or newly certified. Cargo.lock is untouched.
Final git diff --check passed. Only this report and a reference-only note in
Task 510B2 were changed by Task 510B2A; all implementation WIP remains preserved.

No candidate process, inference or Tool execution was initiated: 0 / 0 / 0.
No installed executable was modified or rehashed. Production admission and
preferred baseline constants remain 0.157.1 by current source inspection;
supplied baseline SHA is
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
0.160.0 remains unadmitted, uncertified and not preferred. Candidate expectations
remain the supplied exact path, 0.160.0, size 326872368 and SHA
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.

No authority, dependency, admission, preference, frontend or Tauri change was
made in this task. No authority expansion was introduced; the preserved seam's
full security gates remain incomplete. ADR-B: ADR 0030 and Task 498's existing
error-envelope contract suffice; no new ADR.

No commit, push, exact-head CI, tag, release or version bump. Worktree remains
intentionally dirty. Task 510B must not resume under B. Only a subsequent A
authorizes its exact-artifact schema/deterministic/direct gpt-6.1-sol/Tool round
trip/cancellation/live error-envelope/Desktop turn gates with pre/post hashes;
production admission and preferred baseline updates remain separate work.

Reference-only follow-up: [Task 510B2B](2026-10-04-task-510b2b-typed-certification-verification-causes.md) stopped under E after an unavailable edit interpreter caused an unchanged-source focused rerun; its result and execution mistake are recorded separately.
