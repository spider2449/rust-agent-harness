# Task 333 — Remembered Workspace Catalog Actions and Fresh Re-Admission

## Scope

Implement the backend-only remembered workspace catalog actions and explicit
fresh re-admission bridge. Keep remembered candidates descriptive, preserve
the exact ADR 0027 admission pipeline, create fresh process-local member IDs,
and leave admitted members inert until explicit activation.

## Design

- Replace the startup-only remembered snapshot with a Desktop-owned state that
  serializes descriptive mutation transactions and publishes only after the
  store successfully persists the complete catalog.
- Add closed Tauri actions for catalog presentation, add, update, delete, and
  exact-permutation reorder. Generic DTOs expose only opaque candidate IDs,
  labels, ordering, and location-hint presence.
- Add an explicit remembered-candidate admission action that clones the
  descriptive hint under the catalog lock, releases that lock, and calls the
  existing `admit_repository` function without activation.
- Add deterministic tests for persistence ordering/failure, catalog actions,
  privacy, fresh identity, current repository validation, and catalog/member
  isolation.

## Validation

Run focused remembered-workspace tests, the full `rah-desktop` test suite,
format/check/clippy/diff checks, then commit and push the exact validated head
to `origin/master` and verify exact-head CI.
