# Task 510B2O — Certification seam Clippy recovery and publication closure

Starting HEAD / local origin/master: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserve the complete unpublished WIP and historical reports. Correct only the
nine reproduced lints, inspect the incremental diff, run focused then authoritative
Clippy, typed/presentation/snapshot regressions and one preserved-snapshot smoke,
then fmt/check/workspace/canonical Desktop/frontend/static/Tauri/metadata/authority
gates serially. Stop on required failure. Only A authorizes staging/publication;
natural exact-head CI must pass before Task 510B resume authorization. Do not start
Task 510B, admit 0.160.0, or change preferred 0.157.1.

## Exact pre-edit lint inventory

Command: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
Exit 101; exactly nine denied findings. Full log:
`F:/temp/task510b2o-evidence/clippy-before.log`. Target variables both
`F:/temp/rah-task510b2g-target`. Existing Cargo duplicate-target manifest warning
is separately emitted, not one of the nine denied Rust Clippy findings.

| File (under crates/rah-runtime-codex/src/) | Original line | Lint | Suggestion / planned correction |
| --- | --- | --- | --- |
| certification_support.rs | 102 | result_large_err | Reduce ArtifactMeasurementError; box only measurement payloads |
| certification_support.rs | 225 | result_large_err | Reduce SnapshotError; box only measurement payloads |
| certification_support.rs | 312 | result_large_err | Same SnapshotError representation correction |
| certification_support.rs | 392 | result_large_err | Same ArtifactMeasurementError representation correction |
| certification_support.rs | 400 | io_other_error | std::io::Error::other, same message |
| process.rs | 69 | needless_borrows_for_generic_args | Command::new(executable) |
| process.rs | 95 | needless_borrow | startup_pipe(executable, "stdin") |
| process.rs | 99 | needless_borrow | startup_pipe(executable, "stdout") |
| process.rs | 103 | needless_borrow | startup_pipe(executable, "stderr") |

## Representation decision

R2: ArtifactMeasurementError::Unstable/Incomplete and SnapshotError's
SourceChanged/SourceHash/SnapshotMismatch measurement fields are certification-only
large data. Box those fields only. Keep all function result types, outer adapter
errors, verification errors, source annotations, and neutral contracts unchanged.
The certification module is non-default feature gated. No generic error boxing,
new dependency, display change, hashing/copy/spawn/teardown change is needed.

Task-start Cargo.lock SHA256:
`9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe`.
Its existing unpublished WIP change is preserved; require no drift from task start.

## Corrections and pre-validation guard

Applied the nine listed corrections exactly. Both measurement variants now carry
Box<ArtifactMeasurement>; the four SnapshotError measurement fields carry boxes.
Their constructors wrap the same records in Box::new. No function signature,
source annotation, verification cause, error message, or successful path changed.
Error::other retains "unstable certification measurement" and ErrorKind::Other.
Each of the four process expressions removes only the redundant ampersand.

Saved both pre-edit code files in the evidence directory and inspected their
incremental diffs. Inspected git diff for the complete tracked WIP. The incremental
patch contains only the nine lint corrections; no frontend, snapshot algorithm,
teardown, admission, baseline, neutral API or production error contract change.
No new dependency or Cargo.lock change in this task.

## Gate results and required stop

Final classification: **D — WORKSPACE / DESKTOP / FRONTEND GATE FAILED**.

| Gate | Result |
| --- | --- |
| Focused `cargo clippy -p rah-runtime-codex --all-targets --all-features -- -D warnings` | PASS, exit 0; zero Rust Clippy findings; clippy-focused.log |
| Authoritative `cargo clippy --workspace --all-targets --all-features -- -D warnings` | FAIL, exit 101; clippy-workspace.log |
| Typed / source-chain regression | Not run after required stop; existing typed contracts/source annotations unchanged by inspection, no new runtime PASS claim |
| Sanitized presentation regression | Not run after stop; display/message/frontend mapping unchanged by inspection |
| Snapshot focused tests and sanity smoke | Not run after stop; prior M/N evidence preserved, no current smoke/cleanup/identity certification claim |
| fmt / workspace check / workspace tests | Not run after stop; prior N 1085/0/24 and Desktop 358/0/20 remain historical |
| Canonical Desktop / frontend/static | Not run after stop; no Edge execution or recurrence in this task |
| Tauri inventory / metadata / HostExplicit executable | Not run after stop; no new 47/47/47/47/47, 14-package, or executable 11 closure claim |
| Cargo.lock | SHA256 9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe; unchanged from task start |
| git diff --check | PASS |

Authoritative command reached a previously masked, pre-existing Desktop finding:
`crates/rah-desktop/src/certification_tests.rs:226:5`, `clippy::drop_non_drop`,
`drop(state)`. Argument type: `tauri::State<'_, DesktopAppState>`.
Exact diagnostic: "call to std::mem::drop with a value that does not implement
Drop. Dropping such a type only extends its contained lifetimes".
Clippy supplies no replacement suggestion. Both the certification integration
target and Desktop binary test compilation fail on this one finding.

This file was untouched by O. The finding is not caused by error payload boxing,
Error::other or borrow removal; the initial adapter failure prevented Clippy from
reaching Desktop. The task permits additional corrections only when directly
caused by O, and forbids teardown changes. Therefore no correction, suppression,
retry, downstream gate, staging or publication followed this failure.

## Snapshot, authority, ADR and publication disposition

Preserved path (historical M/N verified identity, not remeasured under O):
`F:/Temp/rah-codex-certification-snapshot-18440-1791116997708427800-0/codex.exe`.
SHA256 `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`,
file ID `0x0000000000000000002e0000000a80a1`, length 326872368.
No artifact recreation or alteration was performed.

Incremental authority/security review: no change to runtime admission, preferred
baseline, ToolRegistry, repository authority/switching, leases, permissions,
Trusted Profiles, mutation uncertainty, remembered-workspace semantics or
provider/model authority. 0.160.0 remains unadmitted/uncertified; preferred
baseline remains 0.157.1. Full publication authority validation remains withheld.
ADR-B: ADR 0005, ADR 0030 and Task 498 diagnostic envelope remain sufficient;
no new ADR. Default production graph support and existing feature gating unchanged.

Committed file count **0**. No staging, commit SHA/message, push, or exact-head CI
run. HEAD and local origin/master remain
`436470337269546a1209d0b9803c2b8aa9e1f6b4`; no remote refresh occurred.
Worktree remains dirty with the complete unpublished Task 510B2 WIP/reports,
including this report. Historical reports remain accurate; N gains only a
reference-only note. No clean-worktree or publication closure claim.

Task 510B resume authorization **WITHHELD**. Task 510B was not begun.
Next task requires explicit bounded authority to address the pre-existing
Desktop drop_non_drop finding while preserving teardown ownership semantics,
then resume the stopped gates. A plus natural exact-head CI remains required.

Reference-only follow-up: [Task 510B2P](2026-10-04-task-510b2p-certification-teardown-lifetime-and-final-publication.md) addresses the state lifetime lint and resumes withheld gates; historical O evidence remains unchanged.
