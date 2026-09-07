# Task 239 — Desktop Explicit Host Tool Invocation Workflow

## Status

IMPLEMENTED — AWAITING EXACT-HEAD CI

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`.
- `HEAD == origin/master == 40b2c7a6279d5a1137eb06dd32349047af1a7409`.
- Task 238 implementation: `6485614671d5a2b920e6ce97949e4f81dc00e637`.
- Task 238 implementation and closure CI: `34075995320` and `34076134983` passed.
- Worktree was clean before Task 239 changes.

## ADR 0021 contract

Host explicit dispatch remains distinct from runtime/model dispatch and from
capability authorization. The Desktop backend owns the closed first-party
eligibility table, typed input forms, currentness checks, D2 admission, and
HostExplicit lifecycle. Existing Tools remain authoritative for capability
policy and result semantics.

## Current composition retention

The connected-current Desktop composition retains the exact runtime registry,
expected definitions, permission policy, and four-generation currentness tuple.
Host commands use that published composition and never reconnect, recompose, or
construct a second registry.

## Shared permission policy

The single host-owned `desktop_allowed_permissions` result feeds both Codex
runtime bridge configuration and the retained connected composition.

## Backend eligibility

The exact first-release allowlist is `fs.read`, `repo.file-info`,
`repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.create-branch`.
External providers, `echo`, `repo.commit`, and other mutation Tools remain
unavailable for host invocation.

## Typed read forms

Read-only commands accept a closed serde enum with bounded path forms for
`fs.read` and `repo.file-info`, and empty typed forms for the three no-argument
repository observation Tools. Tool names and arbitrary JSON are backend-only.

## Branch prepare/review/confirm

Branch creation follows prepare, sanitized review, explicit confirmation,
revalidation, and authorized dispatch. Prepare performs no Tool or Git effect.

## Ticket lifecycle

Branch tickets are opaque, process-local, in-memory, single-use, bounded-TTL,
Tool/input/composition/currentness-bound values. Cancellation and expiry remove
the ticket without dispatch; confirmation consumes it before revalidation.

## Model/host execution coordinator

One private Desktop coordinator atomically excludes ModelTurn, HostPrepared,
and HostRunning ownership. Terminal paths release ownership, and prepared host
work blocks model-turn start.

## HostExplicit lifecycle

Host activity is Desktop-private, source-labelled `host_explicit`, bounded, and
separate from `AgentEvent` and conversation persistence. Active Tool work is
owned by the backend after start.

## D2 dispatch path

Every host route performs `authorize_tool_dispatch` before Started and uses
`authorized_tool_dispatch` for the owned execution revalidation. No production
host route completes authorization with raw `ToolRegistry::execute`.

## Repository result handling

Host results reuse the existing capability-specific branch classification and
sanitized ToolOutput presentation. Underlying ToolError remains distinct from
D2 rejection and possible effects remain conservative.

## Review/currentness semantics

Safe branch creation preserves existing review authorization and generation
semantics. Uncertain results invalidate review and refresh only the captured
repository context when it is still selected.

## Effective Authority

Effective Authority exposes an observational backend-owned host invocation
descriptor and bounded unavailable reasons. The descriptor is never dispatch
authority.

## Frontend UX

Effective Tool rows expose typed host forms and a non-model Host action. The
frontend renders bounded output text/JSON safely and never infers eligibility
from names, permissions, or effect class.

## Tauri permissions

Only the narrow typed host commands are added to the generated command
permissions and default capability allowlist.

## External/repo.commit deferrals

External MCP/Process Plugin Tools, `repo.commit`, and all other worktree or
history mutation Tools remain unavailable and receive no generic host form.

## Cancellation/shutdown

Only prepared work can be cancelled. Tickets and in-flight host authority are
not persisted; shutdown drops prepared work and does not replay started work.

## Deterministic tests

Tests cover exact eligibility, connected-current and permission rejection,
typed read dispatch, branch prepare/confirm/ticket binding, model/host
exclusion, no event forgery, result handling, and frontend authority rules.

## Security nonclaims

HostExplicit is not model execution. A click, visibility, Execute permission,
Effective Authority snapshot, frontend state, or model text creates no
capability authority. This workflow does not add arbitrary JSON, external
provider invocation, commit invocation, post-start cancellation, rollback,
replay, crash-effect claims, OS sandboxing, or network isolation.

## Validation

Sequential validation passed locally:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test -p rah-tools authorized_dispatch -- --nocapture` (15 passed)
- `cargo test -p rah-runtime-codex` (84 passed, 1 ignored)
- `cargo test -p rah-desktop` (189 passed, 1 ignored)
- `cargo test --workspace` (all package tests passed; ignored tests remained ignored)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `node --check crates/rah-desktop/frontend/status.js`
- `node crates/rah-desktop/frontend/status_authority_test.js`
- `cargo build -p rah-desktop --release`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1` (13 packages, all `0.19.0`, edition `2024`)

The Windows-target Desktop compile and focused host-invocation tests also
passed. No live model or Windows certification run was performed.

## Files changed

- `crates/rah-desktop/src/host_invocation.rs`
- `crates/rah-desktop/src/main.rs`
- `crates/rah-desktop/src/effective_authority.rs`
- `crates/rah-desktop/frontend/status.js`
- `crates/rah-desktop/frontend/status_authority_test.js`
- `crates/rah-desktop/build.rs`
- `crates/rah-desktop/capabilities/default.json`
- `crates/rah-desktop/permissions/autogenerated/host_invoke_read.toml`
- `crates/rah-desktop/permissions/autogenerated/host_prepare_repo_create_branch.toml`
- `crates/rah-desktop/permissions/autogenerated/host_confirm_tool_invocation.toml`
- `crates/rah-desktop/permissions/autogenerated/host_cancel_tool_invocation.toml`
- `docs/plans/2026-09-07-desktop-explicit-host-tool-invocation-workflow.md`

## Implementation commit

IMPLEMENTED — AWAITING EXACT-HEAD CI

## Exact-head CI

To be recorded after the implementation commit and exact-head CI.

## Closure commit

To be recorded after implementation exact-head CI passes.

## Final CI

To be recorded after the documentation closure commit.

## Next task

Task 240 — Windows Live Explicit Host Tool Invocation Certification. Not
started automatically by Task 239.
