# Task 460 — Desktop Repository-Selection Lifecycle UX Correction

Date: 2026-09-28. Type: existing-product lifecycle UX maintenance.

**Classification: maintenance correction of an existing repository-selection
lifecycle UX contract.** No v0.33 capability is selected.

## Checkpoint and diagnosis

- Starting `HEAD` and `origin/master`: `05abef3238e6af7b63671f0bb79c07a7e2ae58e8`.
- Starting worktree: clean, in isolated checkout `rah-task-457`.
- Task 458 recorded that repository selection returned “Repository selection is
  unavailable while chat is running” while Chat showed “Chat ready”. Explicit
  Disconnect, repository switching, and reconnect allowed the task to continue.
- Task 459 traced the lifecycle mismatch: `ConnectionState` is host-owned and
  `app_status` publishes its lifecycle state. The backend already admits
  repository selection only in `NotConnected` or `Error`, but the frontend
  chooser omitted `connected` from its disabled states and mapped connected
  rejection to the actual-chat-running error. Active-member activation was
  already disabled while connected.
- Accepted ADR 0027 continues to define descriptive membership, host-controlled
  explicit activation, zero-or-one active repository, and active-only executable
  composition. Task 460 does not alter that boundary or HostExplicit.

## Intended correction

Use the published canonical connection lifecycle state for chooser and member
activation presentation. Keep selection enabled only for the backend's
disconnected states (`NotConnected` and recoverable `Error`), preserve the
separate active-turn guard, explain that Disconnect is required while
connected, and make the host's connected-state rejection accurately identify
that requirement. No automatic Disconnect or reconnect is introduced.

## Implementation and evidence

### Source correction

- `crates/rah-desktop/frontend/status.js` now uses the `codexStatus` value
  published by `app_status` to gate both Choose Repository and active-member
  activation. `connecting`, `connected`, and `disconnecting` block selection;
  `not connected` and the host's recoverable `error` state remain selectable.
  Unknown status fails closed. A separate polite hint says to Disconnect while
  connected and to wait through connection transitions. Active turns retain a
  distinct explanation.
- `crates/rah-desktop/frontend/index.html` starts the chooser disabled until
  canonical status arrives and provides the lifecycle hint.
- `crates/rah-desktop/src/main.rs` retains `RepositoryBusy` for the actual
  `ChatState` guard and returns `RepositorySelectionRequiresDisconnect` for
  connection lifecycle rejection. The guard still permits only
  `NotConnected` or `Error`.
- Frontend coverage in
  `crates/rah-desktop/frontend/repository_membership_test.js` covers all
  required lifecycle values, the connected-ready / Chat-ready Task 458 state,
  active-turn messaging, Disconnect re-enablement, and recoverable Error.
- `status_authority_test.js` also passed as an existing Desktop frontend
  regression.
- The deterministic host regression in
  `crates/rah-desktop/src/main_tests.rs` directly invokes member activation
  while connected, verifies the sanitized error, preserves active A and its
  connected composition/registry, then transitions the fake runtime through
  shutdown to disconnected and successfully switches to B. Existing
  disconnected selector coverage remains in place.
- No Remembered Workspaces clipping, staged-state explanation, dependency,
  permission, Tool, authority, ADR, version, or release changes were made.

### Validation and stop boundary

- Focused host test passed:
  `cargo test -p rah-desktop connected_repository_selection_rejects_direct_activation_without_changing_authority`
  — 1 passed, 0 failed, 343 filtered out.
- Frontend checks passed: `node --check` for `status.js` and
  `repository_membership_test.js`; `repository_membership_test.js`;
  `status_authority_test.js`; and `tauri_permission_test.js` (47 runtime,
  47 manifest, 47 generated, 47 default allows, and 47 frontend commands).
- `cargo fmt --check` reported formatting differences in the new Rust test.
  `git diff --check` emitted no whitespace error diagnostics.
- `cargo test -p rah-desktop` finished with **320 passed, 4 failed,
  20 ignored, 0 filtered out**. Failures:
  - Three `provider_composition` tests stopped because the isolated checkout's
    `target/debug/rah-plugin-echo.exe` and `target/debug/rah-mcp-echo-server.exe`
    fixtures had not been built.
  - `activation_connection_transition_wins_before_publication` still expected
    `RepositoryBusy`; the corrected guard returned
    `RepositorySelectionRequiresDisconnect`.
- Per Task 460's validation stop rule, work stopped after that package failure.
  The canonical Windows Desktop gate, workspace gates, production Desktop live
  scenarios, pre-commit audit, commit, push, and exact-head CI were not run.
  Task 460 is therefore **PARTIAL / STOPPED AT DESKTOP PACKAGE VALIDATION**;
  no clean validation or completion claim is made.

### Authority and repository state

This correction does not change the existing zero-or-one active repository
model, host/user selection, active-only executable composition, restart
behavior, or the exact 11 HostExplicit Tools. RAH stays `0.32.0`; v0.33 remains
**NONE SELECTED**. The worktree contains only the six intended files listed
above and has not been committed or pushed.

## Preserved scope

The Remembered Workspaces location-control clipping and staged-state explanation
remain separate findings from Task 459. RAH remains `0.32.0`, HostExplicit
remains exactly 11, current certified Codex remains `0.157.1`, and v0.33
capability remains **NONE SELECTED**. No release or tag is part of this task.

## Task 461 validation re-entry (2026-09-28)

The original Task 460 package-test failure above is preserved as observed. Task
461 classified and corrected the validation blockers without changing
production lifecycle behavior.

- The three provider failures were fixture-preparation failures from invoking
  `cargo test -p rah-desktop` directly. The tests require
  `rah-plugin-echo.exe` and/or `rah-mcp-echo-server.exe` under the selected
  Cargo target `debug` directory. The direct Cargo command did not build them.
  `scripts/windows-desktop-test-gate.ps1` builds and verifies both helpers
  before running the unfiltered parallel Desktop suite.
- `activation_connection_transition_wins_before_publication` coordinates a
  Connecting transition against repository activation publication. It starts
  no chat turn. Its correct rejection is
  `RepositorySelectionRequiresDisconnect`; the test still verifies that
  repository A remains active and the generation is unchanged. The separate
  actual-turn regression retains `RepositoryBusy`.
- Ran `cargo fmt`; formatting changes were confined to the Task 460 Rust edits.
  `cargo fmt --check` now passes.
- The focused transition-race test and direct-connected-selection host test
  passed. Frontend lifecycle and existing authority/permission checks passed.
- The canonical Windows Desktop gate passed once:
  **324 passed, 0 failed, 20 ignored**. Each of the three provider tests passed
  after the gate prepared its fixtures.
- Workspace deterministic validation passed: `cargo check --workspace`,
  `cargo test --workspace`, clippy with `-D warnings`, frontend regressions,
  `git diff --check`, and Cargo metadata validation (13 packages, 13 workspace
  members, version 0.32.0, edition 2024).
- Production live validation passed using the normal release Desktop and
  certified Codex CLI 0.157.1. While disconnected, the repository chooser was
  actionable and repository A was selected. Connected and idle, the chooser
  was disabled with “Disconnect before changing repositories.” and the UI
  showed “Chat ready”; no chat turn was started. After normal Disconnect, the
  chooser became actionable and repository B became active in the same Desktop
  process (PID 9180); no RAH restart was needed.

Task 461 evidence and the complete gate record are in
`docs/plans/2026-09-28-task-461-task460-desktop-validation-reentry.md`.
At this documentation checkpoint, commit, push, and exact-head natural CI are
still pending; Task 460 completion is not claimed until those gates pass.