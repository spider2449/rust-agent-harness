# Task 509F1 — v4 fixture migration and certification

Reference-only follow-up: [Task 509B-R2](2026-10-04-task-509b-r2-runtime-advertised-picker-custom-freshness-and-connect.md)
uses the published v4 modes in stopped picker/source WIP. R2 classified E after
its first focused compilation failure; it does not alter F1's persistence
certification or establish validated picker/Connect behavior.

Starting HEAD: `8c88bfac36d96bb16958375936a315170a2a6cb8`.
Preserve all stopped Task 509F implementation and documentation WIP.

## Bounded execution plan

1. Audit every `DesktopModelSelection` initializer, default and helper and all
   serialized preference versions; classify no-model T1, normal/legacy T2,
   and explicit Custom T3 before compiling.
2. Correct the known E0063 fixture at `main_tests.rs:16345`; add deterministic
   v4 and legacy semantic coverage without changing product semantics.
3. Freeze Rust source; run the exact failed focused command, consumer tests,
   workspace gates, canonical Desktop, frontend/static, inventory, metadata,
   and static/executable HostExplicit checks. Preserve failure evidence.
4. Only A permits commit, normal master push and natural exact-head CI.
   Do not automatically resume Task 509B-R.

## Constructor and fixture audit

Exact-type searches cover the repository, including parser, defaults,
model-control construction, main tests and runtime-model-state tests.
All existing literal explicit IDs are T2, including invalid-ID fixtures:
their rejection concerns ID/provider/endpoint constraints, not Custom provenance.
The no-model invalid OpenAI fixture and default constructor are T1.
The parser and model-control builder derive mode from the version/explicit ID
and existing selection respectively; both already contain the new field.
The known mixed-provider E0063 site is T1 for Inherit and T2 otherwise:
`(provider != DesktopModelProvider::Inherit).then_some(ModelSelectionMode::Advertised)`.
New explicit Custom fixtures are T3; same-ID tests cover both T2 and T3.
The existing `explicit` helper is renamed `advertised_model_pref` to make
its normal explicit-model semantics unambiguous.

Historical v1–v3 wire fixtures retain their historical schema without mode.
New canonical v4 documents use model-a/advertised and custom-model-a/custom;
same-model is tested with both modes. Inherit has no ID and no mode.

## Failed script disposition

Inspected `F:/temp/task509f-evidence/tests.py`; not rerun. Its default-encoding
read failed on UTF-8 source before writes. Intended files were main_tests.rs,
main.rs and desktop_preferences.rs. Its main.rs helper refactor is unnecessary
for this migration and is not applied. Direct edits preserve UTF-8; no non-UTF-8
fixture conversion or encoding churn is needed.

## Validation and closure

Evidence: `F:/temp/task509f1-evidence/`. Frozen source hashes: `source-sha256.csv`.

Constructor inventory after migration:

| File/sites | Classification |
| --- | --- |
| main_tests.rs: 23 literals | 21 T2; one T1 invalid no-model OpenAI; one conditional T1/T2 at the known E0063 site |
| runtime_model_state.rs: 2 literals | T2 advertised configured/restored Codex preferences |
| desktop_preferences.rs: helper plus 3 endpoint literals | T2, including invalid endpoint rejection fixtures |
| desktop_preferences.rs: new round-trip literal | Explicit T2/T3 parameter matrix |
| desktop_preferences.rs: parser literal | Version-aware T1/T2/T3, no default inference of Custom |
| main.rs: Default Self constructor | T1 |
| main.rs: model-control literal | Explicit ID preserves restored mode, otherwise new ID Advertised; no ID no mode |

Exact-name/builder searches found no other constructor-bearing files;
codex_composition.rs contains an implementation block, not a constructor.
Default call sites retain T1; no ambiguous helper default was introduced.
Serialized fixtures are inline Rust documents; there are no external fixture
files requiring encoding conversion.

The first retry failed compilation on five namespace references introduced in
new fixture code (E0433), not product code. Corrected all references together.
The second run executed 27 tests: 26 passed / 1 failed / 0 ignored, exit 101.
The failure was the audited canonical-writer expectation still using v3 bytes.
Preserved it as a historical v3 parser input and added exact v4 canonical bytes;
renamed the v1/v2 save-upgrade test to describe v4 accurately. No product change.
Both intermediate logs remain `focused-preferences-attempt1.log` and
`focused-preferences-attempt2.log`.

Final exact command:
`cargo test -p rah-desktop desktop_preferences::tests -- --test-threads=1`.
Both target variables use `F:/temp/rah-task509a2-target-run2`.
**27 passed / 0 failed / 0 ignored; exit 0**.

v1/v2/v3 explicit IDs normalize to Advertised, including the explicit absent-ID
regression using a hypothetical catalog; legacy schema documents contain no mode.
v3 identity/profile data migrates intact. Canonical model-a/Advertised and
custom-model-a/Custom round-trip exactly; same-model persists distinctly in both.
Versions 1–4 Inherit restore no ID/no mode and write the exact v4 Inherit shape.
Missing/unknown/null/numeric/duplicate modes, mode without model, invalid Inherit,
and model/root unknown fields reject. Legacy documents reject the v4-only field.
Legacy and v4 persisted openai map through `codex_model_config` to Codex upstream
OpenAI; runtime-model-state consumer tests retain native runtime separation.

Existing consumer filters passed: preference 38/0/0; desktop_model_selection
2/0/0; runtime_model_state::tests 2/0/0. Executable HostExplicit allowlist
1/0/0; static closed host_kind match has exactly 11 named arms.
Existing runtime-model-state and model-preflight frontend tests passed.

Workspace gates: `cargo fmt --check`, `cargo check --workspace`,
`cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
and `git diff --check` all passed, exit 0. Summed workspace test/doc-test
counts: **1068 passed / 0 failed / 24 ignored**. Workspace Desktop segment:
341/0/20. Logs and exit files are retained under the evidence directory.
Echo helpers used the canonical packages/binary flags. An initial preparatory
command confused binary names with package names and exited 101 before any
build; corrected only the command and retained `fixtures-command-error.log`.
No stale/corrupt target behavior occurred; no isolation retry was needed.

Canonical command:
`powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:/temp/rah-task509a2-target-run2 -OutputDirectory F:/temp/task509f1-evidence/desktop`.
Run `20261004-134030-371-2d126485dbf54a4d95d0c7918dd338e3`:
**341 passed / 0 failed / 20 ignored; exit 0**; helper build exit 0;
no watchdog timeout. Executable HostExplicit allowlist passed in this gate too.

Complete frontend/static PASS, exit 0: syntax checked every frontend JS file
and tauri_permission_test.js; executed all six frontend test files (model
preflight, remembered-workspace browser layout, remembered-workspace behavior,
repository membership, runtime-model-state and effective authority).
`node crates/rah-desktop/tauri_permission_test.js` PASS, exit 0:
**47 matching commands** across runtime handlers, manifest, generated permissions,
default allows and frontend. No permission drift.
Metadata verified 14 workspace members/packages, all 0.33.0;
Cargo manifests and lockfile have no diff, no unintended dependency drift.

ADR-B: private inactive preferences retain existing ownership under ADR 0015.
Architecture guardrails exclude routine implementation details from new ADRs;
ADR 0028's separate remembered-workspace schema is unchanged. No authority,
dependency, version, lifecycle or permission expansion is authorized.

## Final classification and publication

**A — PREFERENCES V4 SAFELY PERSISTS EXPLICIT MODEL-SELECTION MODE**

All audited constructors and fixtures compile with semantically classified modes;
all required semantic and deterministic gates pass. Task 509F is completed by
this recovery. Four Rust files carry the schema/normalization and test migration;
Task 509F/F1 documentation and the two preserved Task 509B-R documentation
changes accompany the coherent commit. Product semantics were not changed during
F1. Source hashes remained unchanged throughout successful validation.

No public/core API or dependency edge, authority expansion, ToolRegistry bypass,
repository-selection change, permission mutation, Trusted Profile activation,
automatic connection or lifecycle change. HostExplicit remains exactly 11.
Saved mode is inactive provenance, never catalog validity or authority.
No Codex 0.160.0 certification, live OpenAI, picker/freshness/Connect implementation,
tag, release or version bump. Remaining catalog/Connect validity is explicitly
outside this certification; historical readers do not gain v4 support.

Authorized publication: `feat: persist model selection mode`, normal GitHub
master push, no force push, require natural exact-head CI PASS. Publication SHA,
push/CI result and final clean-worktree evidence are returned separately after
the commit to avoid self-referential documentation.

## Exact authorized Task 509B-R resume scope

After publication closure, only runtime-advertised Codex picker, explicit Custom
UI and restored mode, backend source snapshot, generation/request freshness,
stale success/error rejection, and mode-aware Connect eligibility under Task 509E.
Freshly revalidate legacy Advertised IDs; never infer Custom from catalog absence.
Preserve explicit Custom regardless of catalog membership. Native OpenAI remains
configured-snapshot only, with no presets, Custom IDs or discovery. No automatic
picker resume; Task 510B/0.160.0 certification, live OpenAI, Task 508L, layout
redesign and authority expansion remain separate.
