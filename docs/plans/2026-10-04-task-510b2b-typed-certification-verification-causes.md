# Task 510B2B — Typed certification verification causes (stopped)

Starting and final HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.

## Plan and intended correction

Preserve existing Task 510B/510B1/510B2/510B2A WIP. Introduce only a non-default
certification-harness, doc-hidden certification verification cause with
MissingArtifact { path }, HashMismatch { expected, actual }, and
VersionMismatch { expected, actual }. Wrap it as an Error::source-bearing
CodexAdapterError variant, retaining RuntimeFailure's existing source chain.
Keep both displays sanitized and test exact typed downcasts plus serialized
RuntimeDiagnostic redaction. First rerun the original focused command; only
then inspect feature graphs, installed controls and real no-turn Desktop smoke,
full deterministic/canonical Desktop gates, metadata/Tauri/HostExplicit, and
publication after classification A. ADR-B: existing Task 498 and ADR 0030 apply.

## E — LATER DETERMINISTIC VALIDATION FAILED

An attempted edit used a Python stdin script, but `python` was not available.
The editing script never executed; no typed cause or assertion correction was
applied. The PowerShell invocation erroneously continued to cargo fmt and the
focused command despite the failed prerequisite. This is an execution mistake,
not a newly diagnosed product regression. No edit occurred while tests ran.

Exact focused command:

```powershell
$env:CARGO_TARGET_DIR='F:/temp/rah-task504-target'
$env:RAH_TEST_TARGET_DIR='F:/temp/rah-task504-target'
cargo test -p rah-runtime-codex --features certification-harness certification_support -- --nocapture
```

Result: exit 1, 2 passed / 1 failed / 0 ignored / 103 filtered out, 0.49s.
Failure: certification_support::tests::exact_factory_version_and_production_separation,
certification_support.rs:216, unchanged assertion expecting RuntimeFailure
Display to contain SHA-256 mismatch. The original lost-evidence defect remains:
verification reduces exact hash/version/path evidence to ProtocolViolation.
RuntimeFailure retains that adapter cause, but exact evidence is already lost.
No new structured source/downcast or redaction result is established.

No retry, installed artifact execution, production rejection control,
wrong-hash/version installed control, candidate smoke, real adapter/neutral
runtime/Desktop connection, shutdown, or pre/post artifact measurement followed.
Task-local inference and Tool execution: 0 / 0. Feature graphs, full workspace
gates, canonical Desktop counts, Tauri 47/47/47/47/47, metadata 14 at 0.33.0,
and static/executable HostExplicit 11 were not rerun. Cargo.lock is unchanged.
No admission/preferred baseline, Tool authority, environment/config/frontend
bypass, dependency or neutral API change was applied. Production 0.157.1
constants remain unchanged by source inspection; installed rejection is unproven
in this task. Security completion remains unvalidated. No new ADR is required.

Only this stop report and a reference-only Task 510B2A note are intentional new
changes. Existing implementation and historical failure evidence are preserved.
No commit, push, CI, tag, release, or version bump. Worktree intentionally dirty.
Task 510B must not resume. A separately authorized correction re-entry must
first actually apply the narrow typed cause/test changes and rerun the original
focused command. Only subsequent classification A permits Task 510B's exact
artifact schema/deterministic/direct gpt-6.1-sol/Tool round-trip/cancellation/live
error-envelope/Desktop turn gates with pre/post hashes. Admission/preferred
baseline changes remain separate; Task 510B is not automatically resumed.
Reference-only follow-up: [Task 510B2C](2026-10-04-task-510b2c-typed-cause-edit-recovery-and-seam-validation.md)
actually applied the typed-cause correction using direct manual patches; edit guard
and original focused tests passed (3/0/103 filtered). It stopped under E at the
subsequent all-features Desktop E0004 exhaustive adapter-error match. Historical
Task 510B2B failure evidence above remains unchanged.
