# Task 510B2D — Certification error presentation mapping and seam closure

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserve dirty Task 510B–510B2C implementation and reports.

Prior failure: E0004 at `crates/rah-desktop/src/codex_composition.rs:11`,
in the exhaustive `frontend_error(&CodexAdapterError)` match. The new
certification-only verification wrapper lacked a presentation arm.

Audit: discovery, version and schema errors have dedicated frontend categories;
configuration, workspace, transport and general connection failures use the
existing bounded `FrontendError::CodexConnectionFailed`. Reuse that category
for `CertificationVerification { .. }` under Desktop `certification-harness`.
No wildcard, private evidence string, payload change or diagnostic DTO change.
The borrowed adapter retains its process-local source chain.

Pre-validation diff guard PASS: explicit feature-gated arm, no wildcard added,
constant frontend enum only. Added a three-cause presentation test asserting
the exact serialized enum and exact source/downcast evidence after mapping.

Validation sequence: exact prior Desktop all-features check, workspace
all-features check, focused verification and presentation tests, feature graphs,
installed controls and exact candidate no-turn smoke, full deterministic gates,
canonical Desktop suite, Tauri/metadata/HostExplicit inventories. Stop at first
failure. Only A permits commit, normal GitHub master push and natural exact-head CI.
No automatic Task 510B resumption; no admission or preferred baseline change.
ADR-B: Task 498 and ADR 0030 remain sufficient; no new ADR intended.

Evidence directory: `F:/temp/task510b2d-evidence`.

## E — LATER DETERMINISTIC VALIDATION FAILED

Exact prior cargo check -p rah-desktop --all-features: PASS.
cargo check --workspace --all-features: PASS.
Focused certification_support tests: 3 passed / 0 failed / 103 filtered out.
Exact source/downcast evidence and sanitized public Display/diagnostic PASS.
Both target variables: F:/temp/rah-task504-target.

Desktop command: cargo test -p rah-desktop --features certification-harness
--test codex-certification certification_frontend_mapping_preserves_private_sources
-- --exact --nocapture. Compilation/link completed in 2m 25s, then executable
codex_certification-d52d592a41ace069.exe failed before tests ran:
0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND (signed exit -1073741511).
Log: F:/temp/task510b2d-evidence/presentation-test.log.
No presentation test count or execution PASS. No retry after this failure.
Future exact filter must use fully qualified name:
certification_tests::certification_frontend_mapping_preserves_private_sources.
Loader failed before filtering; current short --exact filter cannot prove assertion.

Feature graph logs PASS: default-feature-graph.log disables certification;
certification-feature-graph.log explicitly enables adapter certification.
All-features compile exposes support; ordinary CodexFactory::new sets candidate=None.
Only explicit verified construction sets candidate; create re-verifies identity.
No implicit environment/config/frontend/Tauri route or production admission bypass.
These are compile/source evidence, not installed proof.

Installed production rejection, wrong-hash/version/missing-path controls: NOT RUN.
Candidate smoke, exact spawned-path proof, handshake, real adapter/neutral runtime/
Desktop composition, clean shutdown and pre/post hash: NOT RUN.
Candidate remains exact supplied 0.160.0 path and SHA
fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d.
Inference=0; Tools=0; no installed candidate execution in this task.

Full workspace fmt --check/check/test/Clippy and canonical Desktop: NOT RUN
following stop. No workspace/Desktop counts. cargo fmt ran before validation;
final git diff --check PASS. Tauri 47/47/47/47/47, metadata 14/all 0.33.0,
static/executable HostExplicit 11: NOT revalidated. Cargo.lock unchanged.
No dependency edge or version change in this correction.
Production admission/preferred remains exact 0.157.1 by unchanged source and
focused fixture evidence; installed production rejection remains unproven.
No ToolRegistry/repo authority/repo switching/lease/permission/Trusted Profile/
mutation uncertainty/remembered workspace changes. Exact evidence stays local.
No identified authority expansion; complete seam security validation incomplete.
ADR-B: Task 498 and ADR 0030 sufficient, no new ADR.

HEAD unchanged at 436470337269546a1209d0b9803c2b8aa9e1f6b4.
No commit/push/exact-head CI/tag/release/version bump. Worktree intentionally dirty.
Next prerequisite: diagnose preserved Desktop loader failure, then run fully
qualified presentation assertion and outstanding installed/full gates.
Task 510B remains paused. Only A permits exact-artifact schema/deterministic/direct
gpt-6.1-sol/Tool round trips/cancellation/live error-envelope/Desktop turn gates
with pre/post hashes. No automatic resume or production admission update.

Reference-only: [Task 510B2E loader diagnosis](2026-10-04-task-510b2e-windows-loader-entrypoint-diagnosis.md)
reproduces the loader failure in a fresh target and classifies L3 / D: the
certification integration executable lacks the Common Controls v6 manifest
required by its TaskDialogIndirect import. Same-feature binary sibling passes.
Historical Task 510B2D classification and evidence remain unchanged.
