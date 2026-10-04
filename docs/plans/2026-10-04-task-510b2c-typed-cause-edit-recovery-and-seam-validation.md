# Task 510B2C — Typed cause edit recovery and seam validation

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserved dirty Task 510B–510B2B WIP. Previous Python-dependent edit did not execute.

Plan: apply manual patches with source-diff guards, then original focused tests,
feature graphs/all-features compilation, installed controls and no-turn smoke,
full workspace and canonical Desktop gates, Tauri/metadata/HostExplicit.
Stop at first failure; only A permits commit/push/exact-head CI.
Do not resume Task 510B automatically. ADR-B; Task 498 and ADR 0030 suffice.

Edit method: direct apply_patch, no Python. A preceding PowerShell script
construction failed parsing before any edits or tests; replaced with manual patches.
Source files changed: errors.rs, certification_support.rs, Desktop certification_tests.rs.
After every source patch and formatting step, inspected git diff for errors.rs
and git diff --no-index against NUL for preserved untracked source files.
Guard PASS: three structured variants, verifier construction, #[source] wrapper,
old Display assertions removed, typed source/downcast assertions added.
RuntimeFailure -> CodexAdapterError -> CertificationVerificationError.
MissingArtifact retains supplied PathBuf; HashMismatch retains expected/actual
SHA; VersionMismatch retains expected numeric version and reported CLI version.
Public Display and serialized RuntimeDiagnostic remain sanitized.

Original focused command (CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR both
F:/temp/rah-task504-target): cargo test -p rah-runtime-codex --features
certification-harness certification_support -- --nocapture.
PASS: 3 passed / 0 failed / 0 ignored / 103 filtered out; 0.72s, exit 0.
Source/downcast and redaction assertions PASS for all three variants.
Default inverse feature graph: only adapter default; certification disabled.
Explicit Desktop certification feature graph: adapter certification enabled.
All-features source review: ordinary CodexFactory constructs candidate=None;
only feature-gated verified factory sets candidate; create re-verifies exact
identity before schema/app-server. No environment/CLI/frontend/Tauri switch,
mutable global admission or Tool authority added. Compilation pending.
Evidence directory: F:/temp/task510b2c-evidence.

## E — LATER DETERMINISTIC VALIDATION FAILED

First later deterministic gate: cargo check -p rah-desktop --all-features,
with both target variables F:/temp/rah-task504-target. Exit 101.
Exact failure: Rust E0004, non-exhaustive patterns `&_` not covered,
crates/rah-desktop/src/codex_composition.rs:11:11, match on &CodexAdapterError.
The feature-gated new CertificationVerification variant requires an explicit
sanitized FrontendError::CodexConnectionFailed mapping in that exhaustive match.
No correction or retry after this stable-source failure. All-features compilation
is not PASS. Full log: F:/temp/task510b2c-evidence/all-features-check.log.
No source edit occurred while compilation/tests ran.

Installed production 0.160.0 rejection, installed wrong-hash/version controls,
real candidate smoke, app-server handshake, real adapter, neutral runtime and
Desktop composition: NOT RUN. No candidate artifact measurement/startup/shutdown
or pre/post hash result. Inference=0; Tool execution=0 for this task.
Production fixture rejection passed in focused tests; it is not installed proof.
Full workspace fmt --check/check/test/Clippy, canonical Windows Desktop,
Tauri 47/47/47/47/47, metadata 14/all 0.33.0 and static/executable HostExplicit 11:
NOT RUN after stop. No new workspace/Desktop totals or inventory certification.
cargo fmt was executed before all-features check; final git diff --check PASS.
Cargo.lock unchanged. No dependency/version/neutral API/ADR change in recovery.

Admission/preference constants remain 0.157.1 by source inspection and focused
assertions; 0.160.0 remains unadmitted/uncertified. No production wording workaround,
Display sanitization change, environment/CLI/frontend/Tauri bypass, global mutable
admission override or Tool authority expansion was introduced by this recovery.
The full preserved seam security validation remains incomplete. ADR-B; no new ADR.

Final HEAD remains starting HEAD. No commit/push/exact-head CI/tag/release/version
bump. Worktree intentionally dirty with preserved implementation and reports.
Task 510B must remain paused. Recommended next task: narrowly handle the feature-gated
verification wrapper in Desktop's exhaustive frontend error mapping, retaining
CodexConnectionFailed sanitization, then re-enter stopped all-features validation
and installed controls/smoke before full gates. Only subsequent classification A
permits Task 510B exact-artifact schema/deterministic/direct gpt-6.1-sol/Tool
round-trip/cancellation/live error-envelope/Desktop turn gates with pre/post hashes.
Admission/preferred baseline changes remain separately authorized work.


Reference-only: Task 510B2D report (2026-10-04-task-510b2d-certification-error-presentation-mapping-and-seam-closure.md) records sanitized mapping/all-features PASS and subsequent Desktop loader failure. Historical classification unchanged.
