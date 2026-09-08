# Task 250 — Patch Review, Strict Result Hardening, and Frontend UX

## Scope

This task hardens the existing ADR 0022 HostExplicit `repo.patch` workflow. It
adds one shared strict classifier for the current four-field `ToolOutput`
contract, applies conservative terminal handling to the captured repository
context, removes patch source review from generic host activity, and adds the
typed frontend form and ticket-only review flow. No live certification, model
prompt, protocol change, dependency, or runtime-Codex change is included.

## Contract

`classify_repository_patch_output` accepts exactly one JSON object with exactly
`status`, `changed`, `uncertain`, and `reason`, with the status-specific values
emitted by the current `repo.patch` producer. Unknown, incomplete, extended, or
contradictory output is `Malformed`.

HostExplicit patch terminal handling maps verified success to completion,
known precondition and replacement failures to distinct strict classifications,
and uncertainty, malformed output, tool errors, or late dispatch rejection to
conservative states. Repository refresh and reviewed-commit invalidation are
applied only while the captured repository context remains current. No retry,
replay, restoration, or success inference is added.

Patch review data is returned only by the typed prepare command. Patch Prepared
and Started activity events contain no review. The frontend renders the backend
review with `textContent` and `<pre>.textContent`, keeps one in-memory review,
and sends only the ticket ID for confirm or cancel. Escape and dialog dismissal
cancel the backend ticket before local review state is removed.

## Validation

The required validation passed: `cargo fmt --check`, `cargo check --workspace`,
the focused and full `rah-tools` suites (48 focused classifier/patch tests and
219 package tests), `cargo test -p rah-desktop` (189 passed, 5 ignored),
`cargo test --workspace`, clippy with warnings denied, both frontend Node
checks, the release desktop build, `git diff --check`, and metadata validation.
Metadata reports 13 packages, version `0.20.0`, and edition 2024. The Cargo
manifest and lockfile are unchanged. Task 251 Windows live HostExplicit
`repo.patch` certification is not started by this task.
