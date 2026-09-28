# Task 450 — Task 438 safe re-entry audit

Date: 2026-09-28. Disposition: **STOPPED; Task 438 not certified**.

## Preservation and re-entry

- Original dirty source tree remained at `deba6a10d65ce5db9842a77db3420a89cc0201ec` and was not modified.
- Fresh tracked recovery patch: `F:\Temp\rah-task438-pre-task450-tracked.patch`; SHA-256 `87835f1c81c197040dcbeb950265545c3df229fd91e20d75d93d8467dd73b802`.
- Three untracked files were backed up with relative paths under `F:\Temp\rah-task438-pre-task450-untracked`; manifest: `F:\Temp\rah-task438-pre-task450-untracked.txt`. Earlier recovery patches were preserved.
- Fetched `origin/master` and verified exact `379666d0f149c035f7aa7332b7c835aaf308f647`, the published Task 441 maintenance checkpoint (natural push CI `36358219963`, PASS, per task instruction).
- Created clean `F:\Temp\rah-task450-task438-reentry` at that SHA. `git apply --check` passed; tracked WIP applied without three-way/reject handling. All untracked restores had no collision. Diff touched only Task 438 paths, not Tasks 439–449 maintenance corrections. `git diff --check` passed.

## Verified gates before the live stop

- Saved baseline `verify 0.149.0`, `verify 0.157.1`, and `verify-all`: PASS. Resolved 0.157.1 path: `C:\Users\morefunfun\AppData\Local\codex-baselines\0.157.1\codex.exe`; SHA-256 `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
- Deterministic re-entry commands: `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, Tauri permission inventory, frontend syntax and authority tests, `git diff --check`, and `cargo metadata --no-deps --format-version 1`: PASS. Metadata: 13 packages, 13 members, version 0.32.0, edition 2024. The workspace Desktop suite was 321 passed, 0 failed, 18 ignored.
- Canonical `scripts/windows-rah-tools-test-gate.ps1`: one run, PASS, Cargo exit 0. Evidence: `F:\Temp\rah-task450-rah-tools-gate`.
- Canonical `scripts/windows-desktop-test-gate.ps1`: one run, PASS, 321 passed, 0 failed, 18 ignored, no watchdog timeout. Evidence: `F:\Temp\rah-task450-desktop-gate\20260928-075500-434-be78fed77ca24b698fb826b3266fa3a3`.
- Second exact saved 0.157.1 direct control: `codex.exe exec --model gpt-6-luna --sandbox read-only --skip-git-repo-check --ephemeral --json --output-last-message <file> -C <disposable-dir> <neutral-no-Tool-prompt>`; normal exit 0, exact `RAH438_CODEX_01571_OK`, one agent message and turn completion, no Tool item. Evidence: `F:\Temp\rah-task450-direct-control`.
- App-server control: existing `scripts/codex-live-gate.ps1` with exact 0.157.1 hash and `gpt-6-luna` ran `cargo run -p rah-runtime-codex --example live_smoke`. PASS: adapter connection/schema validation, Started, model request, deltas, exact `RAH_CODEX_SMOKE_OK`, Completed, clean shutdown. The isolated home used an ephemeral auth-file copy; no credential content was recorded.

## Production Desktop gate — STOP

Added an ignored Task 450 live test in `main_tests.rs` using the production Desktop Connect and Send commands, explicit `gpt-6-luna`, baseline resolver with no override, and a sanitized Windows process census. `cargo fmt --check` and `cargo check -p rah-desktop --tests` passed after adding it. Its first live invocation failed after Connect and before Send:

```text
Desktop did not spawn exactly one saved app-server executable
test tests::task_450_current_codex_desktop_neutral_chat ... FAILED
0 passed; 1 failed; 339 filtered out
```

The failing assertion combines count and executable-identity checks. Existing source starts `app-server --stdio` after exact version/schema validation, but this run did not retain the sanitized before/after census values. Therefore the exact reason for the census mismatch is **unknown**. This is a process-identity evidence failure; no Desktop chat turn was started, so no Desktop chat success or failure is claimed. The test was not retried or weakened. The real Tool bridge gate, final deterministic certification, commit, push, and exact-head CI were not run.

The WIP remains uncommitted in the Task 450 re-entry worktree. Historical RAH v0.32.0 certification remains exact Codex 0.149.0. Current 0.157.1 certification is **not complete**. HostExplicit remains exactly 11; v0.33 product capability remains none selected. No tag or release was made.

Task 451 later performed one separate Connect-only process ownership census. See `docs/plans/2026-09-28-task-451-desktop-app-server-process-ownership-audit.md`. This reference does not replace the failed Task 450 evidence above.

Task 452 added an ownership-aware, evidence-preserving census and made one new production attempt. That attempt found exactly one Desktop-owned child with the expected hash but failed its independent saved-path comparison. It stopped before neutral Send or Tool observation. See `docs/plans/2026-09-28-task-452-desktop-live-certification-completion.md`. Task 450's historical combined-assertion failure remains unexplained; Task 451 does not prove which old predicate failed.

## Task 454 and Task 455 continuation

Task 454's production Desktop session passed saved executable ownership, Windows file-object identity, SHA256, neutral `gpt-6-luna` chat, and one real `repo.file-info` Tool execution with correct fixture facts and final turn completion. Its deterministic gate stopped on two certification harness Clippy warnings. The historical Task 450-453 failures and classifications above remain unchanged.

Task 455 corrected only those two harness warnings, passed the focused structural regressions and scoped Clippy gate, then passed one new production Desktop session and all full deterministic and canonical Windows package gates. Desktop count: 323 passed, 0 failed, 20 ignored. Task 438 is complete locally and awaits a published exact head with natural push CI before final completion. See the Task 454 and Task 455 plans.