# Task 509B-R8 — Final publication closure

Starting HEAD: `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`.

Plan: fingerprint non-documentation content; remove only R6 trailing whitespace;
compare hashes; review and stage the complete Task 509 set; require both diff
checks; commit, push normally to GitHub master, and require natural exact-head CI.
Verify HEAD equals origin/master and the worktree is clean. Do not start Task 510B.

R7 completed functional, deterministic, IPC, authority and bounded app acceptance.
Its final staged diff check failed (exit 2) only on the preserved R6 compiler
excerpt. R8 removes spaces/tabs only at lines 199, 206, 212, 218, 248 and 249 in
`2026-10-04-task-509b-r6-model-configuration-tauri-lifetime-recovery.md`.
Original diagnostics, line layout and historical failure classification remain.

All 251 non-documentation file SHA-256 values match before/after R8; the 12 R7
implementation/test fingerprint entries also match. Evidence, byte-preserved R6
original, manifests and publication receipt are under `F:/temp/task509br8-evidence/`.
No product/test edit or functional/manual rerun is needed. Cargo.lock, preference
schema, permissions and capabilities are included in the unchanged fingerprint.
The R7 acceptance script is an intentional maintained harness, not temporary output.

Publication remains contingent on staged diff PASS, commit/push success and exact
head CI PASS; the final receipt records SHA, message, CI run and clean state without
requiring a second publication commit. Only that closure supports Task 509 result
A — RUNTIME-ADVERTISED MODEL PICKER AND CONNECT GATING VALIDATED, and final
A — TASK 509 COMPLETE AND PUBLISHED. No success is claimed ahead of those gates.

ADR-B; no new ADR. HostExplicit exactly 11; Tauri 47/47/47/47/47.
R3 production sha2 workspace dependency is preserved. No authority expansion,
runtime certification, baseline change, version bump, tag or release.

After final A, Task 510B is authorized solely for ADR 0030 Codex certification:
exact 0.160.0 artifact/hash/schema audit; deterministic adapter regressions;
gpt-6.1-sol direct and Desktop certification evidence; unknown-version fail-closed
preservation; explicit review before admitted/preferred baseline changes, with no
automatic admission/baseline update outside its certification gate.
