# Task 509B-R4 — Custom provenance rendering and final validation

Starting HEAD: `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`.
Initial state: 14 modified tracked files, five untracked files, nothing staged.
All R2/R3 WIP and failure evidence preserved.

## Plan

1. Audit production/test bytes before editing and identify R1/R2/R3/R4.
2. Correct only provenance presentation; rerun the exact picker test first.
3. Resume frontend/static and inventory gates; stop at the first failure.
4. Only after all gates pass, run remaining metadata, authority, bounded manual,
   workspace and canonical Desktop validation. Only A permits publication.

## Byte audit and correction

R3 failed at model_picker_test.js:41: expected `Custom · unverified`, actual
`Custom ? unverified`. R2 classification: status.js renderModelSource directly
contained `Custom ? unverified. ` (separator byte `3F`, U+003F). Its picker
selection handler also contained that literal. No helper transforms the value:
both paths assign textContent directly.

The test expectation contains `/Custom · unverified/`, with separator bytes
`C2 B7`, U+00B7. Strict UTF-8 decoding of both complete files succeeded; neither
contains U+FFFD. PowerShell's default Get-Content display showed mojibake for
the test, but strict byte decoding confirmed its correct expectation. The first
Node inline audit command failed because Windows PowerShell stripped argument
quotes; no source mutation occurred. The subsequent .NET strict decoder and
byte inspection established the source values independently of terminal text.

The corrupted literals already exist in the stopped R2 source. The precise
historical write operation introducing them was not established; current evidence
does not prove a frontend/build encoding boundary. No repository normalization
or ASCII fallback is warranted.

Only status.js changed: replace `Custom ? unverified` in both presentation paths
with `Custom · unverified`; replace the associated runtime and Inherit provenance
` ? provider compatibility` separators with ` · provider compatibility`.
Write used explicit strict UTF-8 without BOM, preserving existing line endings.
No test expectations or picker behavior changed. Other question-mark labels were
left untouched because they are outside this provenance correction.

## Executed validation and stop

Evidence directory: `F:/temp/task509br4-evidence/`. First exact rerun:
`node crates/rah-desktop/frontend/model_picker_test.js`, exit 0, PASS
(`first-picker.log`, `first-picker.exit`).

All nine JavaScript syntax checks passed (`node --check`): eight frontend files
and tauri_permission_test.js. All seven frontend suites passed: model_picker,
model_preflight, remembered_workspace_layout (three Edge viewport fixtures),
remembered_workspace, repository_membership, runtime_model_state, status_authority.
Combined frontend/static execution exited 0. These scripts report suite success,
not numeric assertion totals. No assertions were weakened.

Picker execution preserves Advertised absence blocking, explicit Custom action
and restoration, Inherit, native configured-only/None, mode-aware Connect gating,
connected immutability, loading withdrawal and stale success/error rejection.
The absent `gpt-6.1-sol` assertion passed. Runtime-state tests also passed adapter
scoping, return to Codex and stale-model fixtures. Backend snapshot/freshness,
context switching, compatibility reset and persistence were not modified.

`node crates/rah-desktop/tauri_permission_test.js` exited 0 and printed:

```text
Tauri permission inventory passed: 48 runtime, 48 manifest, 48 generated, 48 default allows; 48 frontend commands
```

This violates the explicit R4 required inventory of **47 matching commands**.
The additional R2 command is `model_source_snapshot`, present consistently in
handler, build manifest, generated permission, default capability and frontend.
No wildcard or mismatched permission was reported. This is a task requirement
failure despite the inventory script's internal consistency PASS. No command or
permission was removed to force 47; no second implementation correction occurred.
Validation stopped at this first later failed requirement.

## Preserved evidence and unexecuted gates

Comparison against R3 source-freeze.txt confirms every frozen implementation
file except the intentionally corrected status.js remains byte-identical,
including all Rust, Cargo manifest and picker-test files. D2 remains the direct
production `rah-runtime-codex -> sha2` dependency; no D1/D2/D3 reconsideration.
Cargo.lock has no diff. Fresh metadata/dependency graph validation was not reached:
14 members/all 0.33.0 is a required gate, not a new R4 certification.

Prior R3 evidence remains: focused picker **17/0/0**, workspace **1085/0/24**,
canonical Desktop **358/0/20**, exit 0; fmt/check/clippy/diff PASS. Prior
HostExplicit static/executable exactly **11** remains unchanged source evidence.
These are preserved R3 results, not freshly executed R4 Rust/Desktop gates.
Fresh HostExplicit, metadata, bounded manual acceptance, final workspace closure
and canonical Desktop rerun were not reached. No manual real 0.157.1 catalog,
Connect, malformed Custom, persistence, switching or native Desktop acceptance
is claimed. No live OpenAI request or Codex 0.160.0 certification occurred.

ADR-B; no new ADR. R4 introduces no dependency, authority or lifecycle change.
ToolRegistry, repository authority/switching, leases, permissions, Trusted
Profiles, mutation uncertainty and remembered-workspace code remain untouched.
The preserved R2 read/refresh IPC permission causes the 47-versus-48 discrepancy;
it is not a new R4 grant or an executable Tool permission. No authority/lifecycle
regression was demonstrated, but incomplete gates prevent final certification.

## Final disposition

**E — LATER VALIDATION FAILED**

Task 509 is incomplete. No commit, push, CI, tag, release or version bump.
HEAD remains the starting SHA. Worktree remains dirty with all original WIP,
the provenance fix and R4 documentation preserved; nothing staged.
Closure git diff --check passed. The next recovery must resolve the explicit
inventory requirement against the preserved model-source IPC before resuming
remaining gates. Task 510B is not authorized to start.

Exact deferred Task 510B scope: runtime certification under ADR 0030, exact
artifact/schema audit, deterministic regression, direct and Desktop certification
evidence, and separately authorized admission/baseline update. No general UI
redesign or live native OpenAI work; R4 does not begin it.

R5 continuation reference only: see
[Task 509B-R5](2026-10-04-task-509b-r5-tauri-model-source-snapshot-boundary.md).
R4 evidence and classification above remain unchanged.
