# Task 076 — Codex Baseline Manager

## Scope

Implement Windows x64 user-level certified Codex baseline management and reuse
the existing explicit `CodexRuntime` executable argument for live gates.

## Delivery

1. Add `scripts/codex-baseline.ps1` with `save`, `verify`, `path`, `list`, and
   `verify-all`; its default store is `%LOCALAPPDATA%\codex-baselines` and it
   supports `CODEX_BASELINE_HOME`/`-StorePath`.
2. Save only an exact native `codex.exe`, acquire it from an isolated exact npm
   package where possible, verify its reported version and SHA-256 before and
   after copy, and persist a closed manifest.
3. Fail closed for an existing version with a different SHA-256; never archive a
   shim or commit a binary to Git.
4. Document daily/candidate/certified tiers, host-only authority, promotion, and
   release-gate use of the verified explicit path.
5. Reuse `CodexRuntime::connect(executable)` / `connect_tool_bridge(executable,
   ...)`; no provider-neutral RAH boundary or new runtime authority changes.

## Verification plan

Run the script against an isolated store, prove idempotence and collision
rejection, upgrade global Codex only after the certified baseline verifies, run a
live smoke with `RAH_CODEX_EXECUTABLE` set to `path`, then run the focused and
full workspace gates. Ubuntu CI validates Rust/docs only, not Windows baseline
archiving.
