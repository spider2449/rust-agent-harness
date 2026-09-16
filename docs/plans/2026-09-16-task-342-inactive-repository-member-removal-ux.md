# Task 342 — Inactive Repository Member Removal UX

## Scope

Expose the accepted Task 341 inactive-only `remove_repository_member` command
through the existing Desktop admitted-repository membership control. The
frontend will keep one authoritative membership snapshot, require explicit
confirmation, submit only the opaque current member selector, and render the
sanitized membership snapshot returned by the backend.

This task does not add repository authority, change backend lifecycle
semantics, delete files, forget remembered candidates, activate or switch
repositories, persist member IDs, add dependencies, or change HostExplicit,
provider, model, profile, or Codex behavior.

## Implementation

- Add exactly `allow-remove-repository-member` to the default Tauri capability.
- Reuse the existing membership selector and render explicit Active/Inactive
  status in its options.
- Add a selected-member Remove control that is enabled only for an inactive
  member. Explain why the active member cannot be removed.
- Add a native dialog whose confirmation text distinguishes process-local
  membership from repository files and remembered workspace entries.
- On confirmation, invoke only `remove_repository_member` with `{ memberId }`.
- On success, render the returned membership snapshot. On bounded backend
  failures, show safe text and refresh membership once without retrying or
  changing lifecycle state.
- Keep cancellation effect-free and preserve the existing remembered,
  repository, activity, and runtime surfaces.

## Deterministic evidence

Extend the dependency-free frontend membership and Tauri permission tests to
cover rendering, explicit confirmation/cancellation, closed invocation input,
returned-snapshot rendering, active/stale/busy mappings, refresh behavior, and
isolation from activation, remembered deletion, filesystem, model, provider,
and persistence APIs. Retain the existing injection and browser-persistence
checks.

## Validation

Run the focused Node checks, `cargo fmt --check`, Desktop check/tests/clippy,
`git diff --check`, and `cargo build -p rah-desktop --release`. Run the full
workspace exact-head CI after commit and push. GUI/manual validation is only
reported if a usable Desktop runtime is actually exercised.
