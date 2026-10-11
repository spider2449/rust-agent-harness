# Task 509F — Preferences v4 model-selection mode migration

Starting HEAD: `8c88bfac36d96bb16958375936a315170a2a6cb8`.
Two intentional Task 509B-R documentation changes are preserved and included.
Certified/preferred Codex remains `0.157.1`.

## Bounded plan and schema

1. Audit existing ownership, closed schema and default semantics.
2. Add an explicit private provenance discriminator and canonical v4 writer.
3. Preserve v1–v3 normalization, endpoint, identity and inert profile-path meanings.
4. Test migration, round trips, malformed state and consumer regressions.
5. Freeze source; run focused, full workspace, Windows Desktop, frontend/static,
   Tauri/metadata and HostExplicit gates. Stop on deterministic failure.
6. Only classification A permits commit, normal GitHub master push and exact-head CI.

v3 cannot distinguish explicit normal from Custom. v4 adds only
`model.model_selection_mode`, serialized as `advertised` or `custom`:

```json
{"version":4,"model":{"provider":"openai","model":"model-a","model_selection_mode":"advertised"}}
```

Inherit remains `{"version":4,"model":{"provider":"inherit"}}`, with no
explicit model or mode. Existing non-Inherit providers require an explicit model;
no new default/provider combinations are introduced. Optional endpoint,
commit_identity and trusted_profile structures retain their current semantics.

v1/v2/v3 explicit IDs normalize in memory to Advertised, including IDs absent
from a future catalog. Legacy Inherit stays absent/default. Loading does not
rewrite the preferences file. Explicit saves emit only canonical v4, retaining
byte limits, atomic replacement and existing bounded warnings.

Persisted `openai` remains Codex upstream OpenAI, never native runtime selection.
Native OpenAI remains configured-snapshot only with no persistent model preference.

Advertised restoration requires future fresh catalog revalidation; absence is
stale/invalid, never Custom. Custom restoration retains explicit provenance even
if a catalog advertises the same ID. Neither catalog nor runtime artifact is a
persistence input. Existing provider updates retain restored mode, and existing
controls create only Advertised for new explicit preferences. Future explicit
picker action owns creation of Custom; this task adds no picker or gating.

v4 requires a mode exactly when an explicit model exists. Unknown mode, missing
mode, mode without model, invalid Inherit combinations, duplicate/unknown fields,
null and invalid types fail closed. Historical versions reject the new field.
The internal state stores an optional typed mode alongside the optional model,
with validation enforcing their structural agreement.

## Validation and disposition

Pending focused and deterministic gates; results will be recorded before closure.
Evidence directory: `F:/temp/task509f-evidence/`.

## Authority, ADR and exact resume scope

ADR-B: bounded evolution of existing private inactive preference storage under
ADR 0015 and Task 509A ownership. Architecture guardrails require ADRs for
architecture changes, not routine implementation; no policy requires a new ADR
for each preferences version. No new ADR, dependency edge or public/core API.

No ToolRegistry, repository authority/switching, leases, Trusted Profiles,
permissions, mutation uncertainty, remembered-workspace or runtime lifecycle change.
Mode describes user-selection provenance and grants no authority. Connected
composition retains its captured configuration; there is no hot recomposition.
HostExplicit must remain exactly 11, verified statically and executably.

After A and publication closure, resume only Task 509B-R's runtime-advertised
Codex picker, explicit Custom UI, restored mode, backend source snapshot,
generation/request freshness, stale success/error rejection and mode-aware Connect
eligibility under Task 509E. Revalidate legacy Advertised IDs against fresh runtime
catalogs; never infer Custom from absence. Preserve explicit Custom regardless
of catalog membership. Native OpenAI remains configured-snapshot only: no presets,
Custom IDs or discovery. Task 510B/runtime 0.160.0 certification, live OpenAI,
Task 508L, layout redesign and authority expansion remain separate. No picker
implementation is resumed automatically by this task.

## Stopped deterministic validation evidence

**E — DETERMINISTIC VALIDATION FAILED**

The implementation is incomplete and unvalidated. The test-extension helper at
`F:/temp/task509f-evidence/tests.py` failed before changing any file because
Python used cp1252 for an existing UTF-8 source file (`UnicodeDecodeError`).
The shell continued to the focused Cargo command against the incomplete patch.
No correction or retry follows this stable-source failure.

Executed with `CARGO_TARGET_DIR` and `RAH_TEST_TARGET_DIR` both set to
`F:/temp/rah-task509a2-target-run2`:

```powershell
cargo test -p rah-desktop desktop_preferences::tests -- --test-threads=1
```

Cargo exit **101**, compile failure:

```text
error[E0063]: missing field `model_selection_mode` in initializer of `DesktopModelSelection`
     --> crates\rah-desktop\src\main_tests.rs:16345:25
```

The failure is an incomplete source fixture migration, not Windows stale-target
or linker contamination. The compiler identifies one initializer. Focused test
counts: **0 executed**; no passing migration/round-trip certification. The planned
Task 509F test additions did not land. Existing canonical expectations were
updated to v4, and the existing unsupported-version fixture now uses v5.

Logs: `focused-preferences.log` and `focused-preferences-exit.txt`.
`source-sha256.csv` freezes the four changed Rust source files after failure.
`cargo fmt` ran (formatting operation only). `git diff --check` passed at closure.

Full `cargo fmt --check`, workspace check/test/Clippy, canonical Windows Desktop,
frontend/config regression suites, complete frontend/static, Tauri inventory,
metadata and executable HostExplicit gates: **not run** after the focused failure.
Workspace/Desktop counts: unavailable. Manifests/lockfile are unchanged; no package
version or dependency changes. HostExplicit source is unchanged but there is no
new static/executable 11-name certification. No authority/lifecycle regression
was observed; no new deterministic acceptance claim is made.

v1/v2/v3 advertised normalization, preserved `openai`, v4 advertised/Custom round
trips, same-ID distinction, malformed/unknown rejection and default behavior are
implemented or intended as described above, but **not test-certified**. The
existing model-control constructor preserves restored mode on explicit provider
updates; no helper extraction or new switch tests landed.

Git remains at the starting HEAD. No commit/push/CI, tag, release or version bump.
The worktree intentionally retains four changed Rust files, the two Task 509B-R
documentation files, and this new report; it is not clean. Task 509B-R is not
authorized to resume. The next bounded recovery is to finish the fixture/test
migration (including the exact E0063 initializer), then run the required gates
before any classification A or publication. Picker implementation remains deferred.

Reference-only: bounded fixture recovery and new evidence are recorded in Task 509F1, docs/plans/2026-10-04-task-509f1-v4-fixture-migration-and-certification.md. Stopped evidence above remains unchanged.
