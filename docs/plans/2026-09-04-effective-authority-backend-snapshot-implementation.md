# Task 193 - Effective Authority Backend Snapshot Implementation

Date: 2026-09-04
Status: implementation complete; awaiting commit and exact-head CI
Starting HEAD: `279fd1aee9b0465a5782c4ca5c4a9c33e580c2df`

## Scope and contract

This task implements only the read-only backend DTO and Tauri command required
by Task 192. It does not implement the frontend panel, authority mutation,
persistence, provider I/O, or lifecycle actions. The Task 192 contract is the
authoritative source for the sanitized fields, closed states, and currentness
rules.

## Design

`rah-desktop` owns `EffectiveAuthoritySnapshot` and its closed enums. Desktop
composition creates the `ToolRegistry` and sanitized classification metadata
together. The published `ConnectionState::Connected` retains that immutable
composition record, so review uses the same inventory that was given to the
runtime. Public names are sorted and private aliases, schemas, handles, paths,
provider diagnostics, and credentials never enter the DTO.

The command is `get_effective_authority_snapshot`, takes no input, and only
copies host state. It reuses `connection_context_is_current` and
`connection_publication_is_current`; only a matching repository/model/
connection publication is `connected_current`. Mismatches are conservative
`reconnect_required` or `stale` states. Repository display uses only the final
directory component. Codex source maps to a closed host label.

The echo Tool is retained in host composition for behavior compatibility but is
classified as a development/test-oriented built-in and is not repository
authority. It is present only when part of a published composition.

Reviewed commit output reuses the existing host presentation enum and exposes
state only. No review is consumed, authorized, revoked, or refreshed.

## Files and permissions

Changed production files are `crates/rah-desktop/src/main.rs`,
`crates/rah-desktop/src/effective_authority.rs`, `crates/rah-desktop/build.rs`,
and `crates/rah-desktop/capabilities/default.json`. The minimal generated
Tauri permission is required by the existing command registration convention.
No ADR, dependency, version, release, or frontend UX file changes are in
scope.

## Tests and validation

Focused tests cover complete-DTO secret scanning, closed sanitized labels, and
Codex source-path exclusion. Desktop lifecycle tests continue to cover the
existing generation/currentness and startup activation counters; the command
contains no provider/runtime, filesystem, Git, persistence, or authority
operation.

Validation completed: `cargo fmt --all`, `cargo check --workspace`,
`cargo test --workspace` (all passed; Desktop 144 passed and 2 ignored),
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo metadata --no-deps --format-version 1` (12 packages, version 0.15.0,
edition 2024, no dependency drift), `git diff --check`, and
`cargo build -p rah-desktop --release`. The complete-DTO serialization leak
test rejects repository paths, executable paths, profile/provider secrets,
stderr, and private aliases while checking expected sanitized labels.

Zero-side-effect behavior is structural: the command only locks/copies host
state, reads immutable composition metadata and workflow presentation, and
performs no provider/runtime construction, provider I/O, Git/filesystem
mutation, persistence, generation increment, Tool execution, or review
authorization/consumption. Existing Desktop startup/currentness tests remain
green.

No frontend panel is implemented, no new authority or ADR is introduced, and
Task 194 is the recommended next task after exact-head CI success.
