# Task 341 — Desktop Inactive Repository Member Removal Foundation

Date: 2026-09-16

## Scope

Implement the host-owned removal of exactly one inactive process-local
repository member. Preserve the Task 340 contract, active-only executable
composition, remembered-workspace isolation, and existing authority/lifecycle
boundaries. This task does not add frontend UX, persistence behavior, an ADR,
new authority, or a dependency.

## Work items

1. Correct `WorkspaceMembershipState::remove` so only a successful inactive
   removal advances membership generation, with explicit removed/active/
   not-found outcomes.
2. Add the closed backend removal function and Tauri command. Parse the
   selector before mutation, acquire membership then lifecycle coordination,
   revalidate current membership and activity, fail closed for target-owned
   lifecycle state, and return the sanitized membership presentation.
3. Register the command and generated permission without enabling it in the
   default frontend capability.
4. Add deterministic primitive, backend isolation/privacy, remembered-catalog,
   busy-state, stale-selector, re-admission, and coordination-race tests.

## Validation

Run focused Desktop tests, all Desktop tests, formatting/check/clippy, the
permission/static test when generated permission output is involved, and
`git diff --check`. Before commit verify exact changed-file scope, clean final
Git state, push `origin/master`, and verify exact-head CI.
