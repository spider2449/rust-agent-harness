# Task 506D — Explicit Codex composition symbol path

Starting HEAD: `fc8d52d631400564083c66c277c1130efed5e3c1`.
Task 506/506A/506B/506C dirty uncommitted WIP preserved.

## Audit and correction

Task 506C removed `use codex_composition::*;` and retained explicit gated
test-helper imports. This exposed one hidden crate-root type reference in
`model_preflight::tests::preflight_gates_connection_factory_before_publication`.
Repository-wide symbol search found no other `crate::PreparedCodexConnection`
reference. Definition and remaining direct uses are in `codex_composition`:
preparation returns the type, connection consumes it, and composition closures
receive it. Its fields are executable, CodexModelConfig and CodexExecutableSource.
It is Codex admission/composition state, not a neutral runtime contract.

Decision **S1 — explicitly module-qualified**. The sole sibling test now uses
`crate::codex_composition::PreparedCodexConnection`. Existing `pub(super)` type
and field visibility is sufficient and unchanged; no external API or root
re-export is added. S2 is unnecessary; evidence does not support S3.

The Windows-only model_preflight module retains neutral production presentation
logic. This test is additionally gated by `provider-codex`; codex_composition
itself is Windows/provider-codex gated. No new cfg or feature-model change.
All three other Task 506C comment lint corrections are preserved.
No runtime body changes, no Task 507/OpenAI code.

## Validation plan

Run warnings-denied workspace/all-target/all-feature Clippy first; stop on a
new unrelated finding. Then disabled Desktop check, default Desktop check,
canonical Windows Desktop gate, frontend/static tests, Tauri inventory,
metadata sanity, HostExplicit static/executable verification and disabled graph.
No source edits during commands. Preserve Task 506B semantic/runtime evidence;
do not repeat its full suites or empty-PATH smoke for this test-only path change.

## Results

All commands completed serially with frozen source. Native Cargo exits, rather
than PowerShell stderr formatting, determine results. Default Cargo target:
`F:/temp/rah-task504-target`; disabled target:
`F:/temp/rah-task506-disabled-target`.

| Gate | Result | Evidence |
| --- | --- | --- |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 | `F:/temp/task506d-clippy.log` |
| `cargo check -p rah-desktop --bin rah-desktop --no-default-features` | exit 0; existing 34 disabled warnings retained | `F:/temp/task506d-disabled-check.log` |
| `cargo check -p rah-desktop --bin rah-desktop` | exit 0 | `F:/temp/task506d-default-check.log` |
| Canonical Windows Desktop gate | PASS; helper/test exits 0; 334 passed, 0 failed, 20 ignored; tests 270.67s | `F:/temp/task506d-desktop-gate/20261003-183705-453-6c965e5447664cc68a50a349490e90db/` |
| Frontend/static | all five suites PASS; status.js syntax exit 0 | task tool output |
| Tauri permission inventory | PASS; 47 runtime, manifest, generated, default allows and frontend commands | task tool output |
| Cargo metadata | exit 0; 13 workspace packages, all 0.33.0; expected default/optional feature edges | `F:/temp/task506d-metadata.json` |
| Disabled dependency graph | exit 0; zero `rah-runtime-codex` entries | `F:/temp/task506d-disabled-tree.log` |
| HostExplicit | exactly 11 static routes; executable exact-allowlist test PASS in canonical gate; disabled executable PASS preserved | canonical stdout and Task 506B disabled log |
| `cargo fmt --check` | exit 0 | `F:/temp/task506d-fmt.log` |

Canonical command: `powershell -NoProfile -File
scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:/temp/rah-task504-target
-OutputDirectory F:/temp/task506d-desktop-gate`. Harness elapsed 326.91s, no
watchdog timeout. Corrected preflight test and default selection test both pass.

Frontend commands: `node` on status_authority_test.js,
repository_membership_test.js, remembered_workspace_test.js,
remembered_workspace_layout_test.js and model_preflight_test.js under
`crates/rah-desktop/frontend/`; `node --check` on status.js; then
`node crates/rah-desktop/tauri_permission_test.js`. Browser layout used Edge.

Metadata command: `cargo metadata --no-deps --format-version 1`; checked
workspace membership/package versions, `default = ["provider-codex"]`,
`provider-codex = ["dep:rah-runtime-codex"]` and optional dependency metadata.
Graph command: `cargo tree -p rah-desktop --target x86_64-pc-windows-msvc
--no-default-features -e normal,build,dev`.

Static HostExplicit routes: fs.read, repo.file-info, repo.status, repo.diff,
repo.diff-staged, repo.create-branch, repo.patch, repo.edit-files,
repo.create-file, repo.delete-file, repo.rename-file. host_invocation has no
feature-dependent route changes and no diff. No Tool authority expansion.

## Preserved runtime evidence and boundaries

Task 506D changes only a gated test type path. Task 506C changed imports/comments,
not runtime bodies. Task 506B final runtime evidence remains applicable:
disabled build PASS, disabled Desktop 307 passed / 0 failed / 11 ignored,
workspace 1,047 passed / 0 failed / 24 ignored, default Desktop within workspace
334 passed / 0 failed / 20 ignored. These are preserved prior runs, not reruns.

Reinspected the retained disabled test and empty-PATH smoke logs. Smoke exit 0
was recorded by Task 506B, with adapter_available=false,
connection_error=runtime_adapter_unavailable, passed=true and runtime_status=
not connected. The disabled executable test proved zero resolver and construction
counters, no configured identity, no panic. Current source still selects None
and routes Connect directly to connect_unavailable before Codex discovery or
admission. No repeat smoke or live GUI/process-observer certification is claimed.

Default selection still chooses Codex; configured identity is the exact preferred
certified constant. Certification, preflight, preferences and admission retain
their behavior. No diff in rah-runtime-codex or its schema, Cargo.lock, permission
declarations or ADRs. No additional dependency direction or public API is added
beyond Task 506's optional existing Desktop adapter edge. ADR-B: ADRs 0030 and
0033 unchanged; the explicit path creates no architectural decision.

## Final classification and publication

**A — DESKTOP BUILD COMPOSITION CAN EXCLUDE CODEX**

Task 506 validation is complete. No glob restored, no widened type visibility,
no runtime correction and no Task 507/OpenAI implementation. Existing three
Task 506C comment lint corrections retained. Starting GitHub master was verified
again as `fc8d52d631400564083c66c277c1130efed5e3c1` before publication.

This report records the pre-commit validation checkpoint. Classification A
authorizes one coherent commit containing Task 506/506A/B/C/D implementation,
tests and reports, normal GitHub master push, and natural exact-head CI closure.
Commit SHA, push/CI identity and final clean status are reported after execution
in the task completion return. No force push, tag, release or version bump.

## Exact Task 507 scope

Native OpenAI adapter core plus deterministic conformance in an isolated adapter
crate, using existing neutral contracts, bounded Responses/SSE/function loop,
typed sanitized failures, owned cancellation and host Tool port. No Desktop UX,
paid live inference or default change. Task 508 owns production composition and
Codex-free operational proof. Classification A authorizes this next scope;
Task 506D does not implement it automatically.
