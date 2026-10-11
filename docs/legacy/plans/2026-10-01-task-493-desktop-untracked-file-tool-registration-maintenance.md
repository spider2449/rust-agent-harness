# Task 493 — Desktop untracked-file Tool registration maintenance

## Starting state and defect

Starting checkpoint: `91b0fa2795587c58c6ef94428ad1a7edc6738b80` (`origin/master`), clean in the isolated `F:\Temp\rah-task493-desktop-registration` worktree. The supplied primary checkout was dirty at `deba6a10d65ce5db9842a77db3420a89cc0201ec` and was left untouched. Task 492 remains historically **B — REPRODUCIBLE MAINTENANCE DEFECT OBSERVED**: production Desktop did not advertise `repo.edit-untracked-file`, so no Tool request or result existed. Its fixture was left with an untracked, unstaged 46-byte `docs/worker-config-correction.txt` containing `worker_count=44` at HEAD `9fa2b9da903264feec592ccef168a80ce256b010`.

The exact omission was `desktop_tool_registry` in `crates/rah-desktop/src/main.rs`: it constructed and registered neighboring `repo.patch`, `repo.create-file`, and `repo.edit-files`, but omitted the existing `RepositoryUntrackedFileEditTool`. Trusted-profile composition and Desktop effective-authority metadata already knew the Tool. The prior trusted-profile dispatch test therefore did not cover this production Desktop registry. The Desktop registry inventory test had a closed expected list that omitted the Tool, allowing the defect to pass.

## Correction and authority

The Desktop registry now constructs and registers the existing `RepositoryUntrackedFileEditTool` using the selected repository's fixed Git executable and root. The selected-repository registry test checks the actual definition, `Execute` permission, composed repository-bound entry, absence of HostExplicit invocation, neighboring registrations, and updated Tool count. No Tool implementation, contract, provider route, dependency, ADR, or authority policy changed. HostExplicit remains the exact 11-Tool set.

Changed files: `crates/rah-desktop/src/main.rs`, `crates/rah-desktop/src/main_tests.rs`, and this report.

## Validation and acceptance

Focused checks: the selected-repository Desktop registry/composition test passed (1/1); the exact HostExplicit allowlist test passed (1/1, 11 entries with explicit rejection of `repo.edit-untracked-file`); the existing trusted-profile composed untracked correction dispatch test passed (1/1). An initial focused invocation used an overly exact test filter and selected zero tests; it was corrected and is not counted as passing coverage.

Full deterministic validation: `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `git diff --check` passed. Workspace tests: 1,010 passed, zero failed, 24 ignored across 55 suite summaries. Canonical `scripts/windows-desktop-test-gate.ps1`: **PASS**, 324 passed, zero failed, 20 ignored, log directory `F:\Temp\rah-task493-desktop-gate\20261002-194421-679-701f465718ee4b9285849db486e9aaf1`. Frontend `status_authority_test.js`, `repository_membership_test.js`, `remembered_workspace_test.js`, and `remembered_workspace_layout_test.js` passed. The first layout invocation failed during headless Edge launch with a renderer error under concurrent compile load; the same unmodified suite passed on retry. `tauri_permission_test.js` passed with 47 runtime, manifest, generated, default-allow, and frontend commands each.

`cargo build --release -p rah-desktop --bin rah-desktop` passed. The resulting `F:\Temp\rah-task493-target\release\rah-desktop.exe` has SHA-256 `06ec425bf85b013cfd97402c4d08a225a33dffb4330d57e77c71a5cf9ed2adab` and was launched as PID 16804. Its RAH window reported version 0.33.0 and Desktop UI ready. The process inherited private evidence path `F:\Temp\rah-task493-production.jsonl`. The user operated the normal production UI; no backend invocation or shell mutation was substituted.

Immediately before the production call, the fixture still had HEAD `9fa2b9da903264feec592ccef168a80ce256b010`, `docs/worker-config-correction.txt` untracked and absent from the index, exact 46-byte preimage SHA-256 `ac4ad6ee537e2f4296690e86b763579a114cdff134d5588737185eb72c32a472`, and `worker_count=44`. The baseline of all fixture file hashes, index hash, and status is retained in `F:\Temp\rah-task493-fixture-before.json`.

The live Desktop log records selected-repository registry composition including `repo.edit-untracked-file`, its dynamic Tool advertisement, `tool_requested`, `tool_started`, and `tool_finished` under one connection and repository generation. Exact request: `{"path":"docs/worker-config-correction.txt","expected_file_sha256":"ac4ad6ee537e2f4296690e86b763579a114cdff134d5588737185eb72c32a472","expected_file_byte_length":46,"replacements":[{"expected_old_text":"worker_count=44","replacement_text":"worker_count=4"}]}`. Exact result: `{"changed":true,"path":"docs/worker-config-correction.txt","reason":"none","status":"ok","uncertain":false}` with `is_error=false`. The user's `F:\rah05.png` UI capture separately shows Requested, Running, Completed and the same result.

Independent post-call inspection found `worker_count=4` present and `worker_count=44` absent. The 45-byte file SHA-256 is `7667772ed74f86f62465f2f0b92b2938b44ec2621975b08296148fb6f4fbac2c`. Git still lists the target as untracked and has no target index entry or staged name. The index SHA-256 remains `bf12f0e8adf364ab242e80d5f77a0fe9d83257705f964bf2e6ab6b6828396ee5`; HEAD and the full porcelain status are unchanged. Comparing every fixture file path, size, and SHA-256 to the pre-call snapshot found only the intended target change and no extra temporary file. The Desktop connected with the selected admitted repository, and its Tool lifecycle records retain the same repository fingerprint and generation. No unrelated file changed.

## Classification and publication

**A — DESKTOP REGISTRATION DEFECT CORRECTED AND PRODUCTION ACCEPTANCE PASSES.** Task 492 remains historically B, resolved by this maintenance result. HostExplicit remains 11; no v0.34 capability is selected. Commit, both normal push results, and exact-head CI are recorded in the Task 493 closure response because a commit cannot record its own SHA or subsequent CI result.
