# Task 203 — Shared Effective Profile Composer Extraction

## Scope

Extract the existing host-owned effective Trusted Profile composer from
`rah-cli` into a narrow shared crate without changing authority, provider
admission, lifecycle, profile schema, runtime behavior, Generic Tool Bridge
behavior, or Desktop composition.

Starting `master` / parent commit:
`4a616cb947bcfbeea1235b66540d5026896986a7` (Task 202).

Task 202 established the architectural requirement: CLI and the future Desktop
provider-only path must share one security-critical effective composition
implementation rather than duplicating provider wiring or making Desktop depend
on the CLI application crate.

## Architecture decision

Add one workspace crate:

```text
rah-tools -----------\
rah-tools-mcp --------+-> rah-profile-composition <- rah-cli
rah-tools-plugin ----/
```

`rah-profile-composition` owns the existing effective Trusted Profile
composition implementation. It sits above the neutral Tool and provider adapter
crates and remains upstream of host applications and downstream of no runtime.

This is the narrow layering already anticipated by the earlier Trusted Profile
MCP design. It is not a new ProviderManager, provider trait, runtime abstraction,
plugin framework, or authority model.

## Code movement contract

The production composer and its deterministic tests move from
`crates/rah-cli/src/profile_composition.rs` to
`crates/rah-profile-composition/src/lib.rs` without semantic edits. The move
preserves the same Git blob so the extracted implementation is bit-for-bit
identical at this task boundary.

Preserved behavior includes:

- fresh `ToolRegistry` construction;
- static-profile first-party capability registration;
- exact MCP and Process Plugin expected-tool/schema admission;
- explicit host permission mapping;
- duplicate-name fail-closed behavior;
- all-or-nothing effective publication;
- staged provider cleanup on late failure;
- provider ownership for the usable registry lifetime;
- repository authority composition hooks already accepted by earlier tasks;
- reviewed commit control retention; and
- bounded redacted effective inventory.

`rah-cli` keeps a compatibility module that re-exports the shared crate. Existing
CLI call sites and opt-in runtime-Codex validation imports therefore keep their
current public Rust path for Task 203 while the implementation ownership moves
out of the application crate.

## Dependency changes

`rah-cli` stops depending directly on `rah-tools-mcp` and
`rah-tools-plugin`; it depends on `rah-profile-composition` instead.

The new composition crate owns the production dependencies on:

- `rah-tools`;
- `rah-tools-mcp`; and
- `rah-tools-plugin`.

Its moved deterministic tests retain only the test dependencies they already
use (`rah-protocol`, `serde_json`, and `tokio`).

No third-party dependency version changes are intended. `Cargo.lock` changes
only to represent the new workspace package and the shifted internal dependency
edges.

Workspace package count becomes 13. Every workspace package remains version
`0.16.0`, Rust edition 2024.

## Explicit non-goals

Task 203 does not:

- add `rah-profile-composition` to Desktop dependencies;
- select, persist, load, validate, or activate a profile in Desktop;
- make MCP or Process Plugin Tools reachable in Desktop;
- add the Task 202 provider-only Desktop profile rule;
- change Trusted Profile v1 schema or source hardening;
- change repository authority or repository selection;
- add profile reload, discovery, persistence, editing, or ProviderManager;
- add network MCP, credentials, generic process, shell, or filesystem authority;
- change Generic Codex Tool Bridge aliases, routing, permission checks, replay,
  cancellation, or disconnect semantics;
- add or modify an ADR; or
- perform a new live certification.

The v0.16 Windows live certification remains the historical live baseline;
Linux live certification remains not established.

## Validation gate

Task 203 is complete only after the exact committed head passes the normal
workspace gates:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`

Metadata must show 13 workspace packages, all version `0.16.0`, edition 2024,
with no third-party dependency drift.

Because the execution environment used to prepare this commit does not provide a
local Cargo toolchain, no local Cargo PASS is claimed here. GitHub exact-head CI
is the authoritative compilation/test evidence for this task.

## Recommended next task

After exact-head CI passes, proceed to Task 204 — Desktop Provider-Only Trusted
Profile Selection. Task 204 should add disconnected-only selection and static
validation without spawning providers or activating new authority.
