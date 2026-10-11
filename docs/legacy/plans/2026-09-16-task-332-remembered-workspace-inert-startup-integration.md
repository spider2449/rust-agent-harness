# Task 332 — Remembered Workspace Inert Startup Integration

Date: 2026-09-16

## Boundary

Integrate the existing `RememberedWorkspaceStore` with the real Desktop
startup owner, `DesktopAppState::new`, using the app-data directory resolved by
Tauri setup. Capture one immutable descriptive startup state:

- valid or missing catalogs become `Available` state;
- corrupt, future-version, and bounded storage failures become `Unavailable`
  status state;
- every outcome leaves repository membership, active repository, repository
  composition, mutation state, provider state, runtime state, and conversation
  repository binding at their normal empty startup values.

## Tests

Add deterministic restart-style Desktop tests for valid, missing, corrupt,
future-version, candidate-location sentinel, inert last-active, prior-process
repository non-restoration, and storage-failure startup. Assert actual empty
authority state and existing startup side-effect counters. Instrument the
remembered location accessor so startup tests prove no candidate hint is
dereferenced.

## Validation

Run focused `rah-desktop` formatting, check, startup tests, clippy, and diff
checks. Commit and push only after local validation, exact `HEAD ==
origin/master`, a clean worktree, and exact-head CI success.
