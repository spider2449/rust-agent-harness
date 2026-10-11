# Task 461 — Task 460 Desktop Validation Disposition and Re-entry

Date: 2026-09-28. Type: bounded validation maintenance.

**Scope:** classify Task 460 validation blockers, correct the stale test
expectation and formatting, then re-enter the specified validation sequence.
No new product capability was authorized or added.

## Checkpoint and starting worktree

- Repository: `spider2449/rust-agent-harness`.
- Starting `HEAD`: `05abef3238e6af7b63671f0bb79c07a7e2ae58e8` (exact expected SHA).
- Starting worktree was intentionally dirty with the six Task 460 files:
  `crates/rah-desktop/frontend/index.html`,
  `crates/rah-desktop/frontend/repository_membership_test.js`,
  `crates/rah-desktop/frontend/status.js`,
  `crates/rah-desktop/src/main.rs`,
  `crates/rah-desktop/src/main_tests.rs`, and
  `docs/plans/2026-09-28-task-460-desktop-repository-selection-lifecycle-ux-correction.md`.
- No reset, stash, clean, rebase, or checkout-over-WIP operation was used.
- Frozen invariants: RAH 0.32.0; HostExplicit exactly 11; certified Codex
  CLI 0.157.1; v0.33 capability NONE SELECTED.

## Independent failure disposition

| Failure | Caused by Task 460? | Expected under direct package invocation? | Production defect? | Test expectation defect? | Fixture-preparation issue? | Action |
|---|---|---|---|---|---|---|
| `mixed_external_providers_are_fresh_composed_merged_usable_and_reaped` | No evidence of a Task 460 cause | Yes: bare Cargo invocation does not build Desktop helper binaries | No | No | Yes; requires both echo helper executables | Use canonical gate, which builds/verifies helpers |
| `activation_rereads_current_source_and_keeps_admitted_snapshot_stable` | No evidence of a Task 460 cause | Yes | No | No | Yes; requires `rah-plugin-echo.exe` | Use canonical gate |
| `changed_source_invalid_or_missing_fails_before_provider_publication` | No evidence of a Task 460 cause | Yes | No | No | Yes; requires `rah-plugin-echo.exe` | Use canonical gate |
| `activation_connection_transition_wins_before_publication` | The implementation behavior is the intentional Task 460 contract | No | No | Yes; expected old `RepositoryBusy` despite no active turn | No | Change only the expected lifecycle error; preserve non-publication assertions |

### Provider fixture evidence

The three failing tests call the Desktop provider fixture helper, which looks
for executable fixtures in the selected target's `debug` directory. The mixed
provider test uses both `rah-mcp-echo-server.exe` and `rah-plugin-echo.exe`;
the other two use `rah-plugin-echo.exe`. The raw `cargo test -p rah-desktop`
invocation did not prepare those executables. The canonical
`scripts/windows-desktop-test-gate.ps1` explicitly builds
`rah-mcp-echo-server` and `rah-plugin-echo`, checks the resulting binaries,
then starts the unfiltered parallel Desktop suite. This proves the correct
classification is **test invocation / fixture-preparation issue**, not a
production regression. No provider test was skipped, ignored, weakened, or
worked around.

Canonical gate evidence, run exactly once after correction:

- Result: PASS; helper build exit status 0; Desktop test exit status 0; watchdog
  timeout false.
- Desktop suite: **324 passed, 0 failed, 20 ignored** (344 total).
- Each of the three provider tests above passed under the canonical gate after
  helper preparation.
- Gate evidence directory:
  `F:\Temp\rah-windows-desktop-gate\20260928-144751-773-1aecd07daa2b4e67a9d80503f9d7f9fa`.
- No second canonical gate run and no final bare package run were performed.

## Repository lifecycle test diagnosis and correction

`activation_connection_transition_wins_before_publication` starts disconnected,
pauses repository activation before publication, requests Connect so the
connection enters `Connecting`, then releases the activation barrier. It does
not start a chat turn. This is a transition race with a connected/connecting
runtime lifecycle, not an active chat turn. Under Task 460, the correct
rejection is `RepositorySelectionRequiresDisconnect`.

Only that expected error changed in this test. Assertions that repository A
remains the active member and that the repository generation stays unchanged
remain intact, preserving the publication/currentness invariant. The separate
`activation_model_turn_transition_wins_before_publication` test starts an
actual turn and retains truthful `RepositoryBusy` semantics. Production
lifecycle behavior was not changed by Task 461.

Ran `cargo fmt`; formatting differences were limited to the intended Task 460
Rust edits. `cargo fmt --check` passed.

## Focused and frontend regressions

All passed:

- `cargo test -p rah-desktop activation_connection_transition_wins_before_publication`
  — 1 passed, 0 failed.
- `cargo test -p rah-desktop connected_repository_selection_rejects_direct_activation_without_changing_authority`
  — 1 passed, 0 failed.
- `node --check crates/rah-desktop/frontend/status.js`.
- `node crates/rah-desktop/frontend/repository_membership_test.js`.
- `node crates/rah-desktop/frontend/status_authority_test.js`.
- `node crates/rah-desktop/tauri_permission_test.js` — 47 runtime, 47 manifest,
  47 generated, 47 default allows, and 47 frontend commands.

## Canonical Desktop and workspace validation

The canonical gate result is recorded above. After it passed, the following
workspace checks also passed:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `node crates/rah-desktop/tauri_permission_test.js`
- `node --check crates/rah-desktop/frontend/status.js`
- `node crates/rah-desktop/frontend/status_authority_test.js`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`: 13 packages, 13 workspace
  members, version 0.32.0, edition 2024.

The workspace Desktop suite reported 324 passed, 0 failed, 20 ignored.
No unrelated validation failures were encountered.

## Production live validation

Built and launched the normal release Desktop from the Task 460 source and used
the current certified Codex CLI 0.157.1. No test-only hooks were used. Two
clean disposable repositories were used at
`F:\Temp\rah-task461-live\A` and `F:\Temp\rah-task461-live\B`.

1. **Disconnected:** runtime showed `not connected`; Choose Repository was
actionable; selecting A succeeded and the selector showed `A (Active)`.
2. **Connected ready:** Connect Codex completed with Codex 0.157.1 and Chat
   ready. No chat turn was started. Choose Repository was disabled and the
   visible explanation was `Disconnect before changing repositories.`
3. **Disconnect and switch:** normal Disconnect returned to `not connected`
   and re-enabled the chooser. Selecting B made the selector show `B (Active)`.
   The same Desktop process remained PID 9180 throughout; no RAH restart was
   needed. The single-active-repository selector therefore no longer showed A
   as active.

The deterministic host regression also verifies that connected repository
selection leaves active A, repository generation, and the connected tool
composition/registry untouched, returns the specific lifecycle error, and
switches successfully after the fake runtime crosses the explicit Disconnect
boundary. Task 461 did not weaken backend defense in depth.

## Documentation and scope

Task 460's original raw package failure and formatting failure remain recorded
above; this report does not rewrite that attempt as green. The Task 460 plan
now contains a separate Task 461 re-entry section. Remembered Workspaces
clipping and staged-state explanation remain out of scope. No dependency,
ADR, permission, authority, version, or release changes were made. No fixture
binaries, target artifacts, logs, credentials, or temporary repositories are
intended to be tracked.

## Git and final disposition

Starting SHA: `05abef3238e6af7b63671f0bb79c07a7e2ae58e8`.

Pre-commit review found only the five Task 460 source/test paths and these two
plan documents. `git diff --check` passed; no fixture binaries, target output,
logs, credentials, or temporary repositories are tracked. The required remote
gate passed: after `git fetch origin master`, both `origin/master` and local
`HEAD` were `05abef3238e6af7b63671f0bb79c07a7e2ae58e8`.

Commit SHA, exact-committed-head validation, push, and natural exact-head CI
remain publication gates at this report checkpoint. Until those gates pass,
Task 460 remains **NOT COMPLETE**.

Upon successful exact-head validation and natural push CI, the intended final
classification is maintenance correction of existing repository-selection
lifecycle UX, not a new v0.33 capability.