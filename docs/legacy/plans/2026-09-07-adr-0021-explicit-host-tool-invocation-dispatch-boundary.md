# Task 236 — ADR 0021 Explicit Host Tool Invocation Dispatch Boundary

## Status

**ACCEPTED LOCALLY — AWAITING EXACT-HEAD CI.** Documentation only; no Tool or
live run, Rust, frontend, Cargo, dependency, or authority implementation change.

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`.
- Required checkpoint: `HEAD == origin/master == a50a58e52b25ca218a9459d96a4c4c1abb6be0c9`.
- Task 235 exact-head CI: `34073078034` PASS.
- Clean worktree; 13 packages, all `0.19.0`, edition 2024, no dependency drift.

## Task 235 contract

The accepted research contract requires ADR 0021, D2 neutral dispatch in
`rah-tools`, connected-current operation, the exact first-party six-Tool
allowlist, typed backend forms, branch prepare/review/confirm/revalidate,
same host-composed permission policy, one-host/no-model concurrency,
`HostExplicit` lifecycle without `AgentEvent` changes, before-start-only
cancellation, no crash persistence or conversation injection, and future live
cases `repo.status` and `repo.create-branch`.

## ADR decision

Created and accepted ADR 0021, **Explicit Host Tool Invocation Dispatch
Boundary**, dated 2026-09-07. It documents only the human-explicit dispatch
route to already-composed Tools and grants no underlying authority.

## Dispatch / authority separation

`host explicit dispatch != runtime/model dynamic dispatch != capability
authorization`. Explicit human intent selects only an eligible, current,
already-composed Tool. Runtime lifecycle remains model-owned, and existing
Tool-specific policy remains authoritative.

## Neutral D2 boundary

Future `rah-tools` D2 re-resolves the public Tool, compares the complete
expected/current definition (name, description, schema, permission), checks
the current host permission policy, and only then calls `ToolRegistry`.
It returns dispatch rejection separately from Tool output. Raw Desktop registry
execution is prohibited as the product path; D2 owns no Desktop, Codex,
provider, ticket, capability, retry, or rollback policy.

## Permission equivalence

Both routes use the same current host-composed `allowed_permissions` policy.
The frontend, ticket, Tool name, Effective Authority, provider metadata, and
`Execute` cannot grant capability authority.

## Connected-current contract

No disconnected execution, silent registry construction, reconnect,
recomposition, profile activation, or authority restore. Current composition,
registry, definition, Effective Authority, repository context, profile/model/
connection tuple, and no reconnect-required state are required.

## Host eligibility

Backend-owned first-party allowlist: `fs.read`, `repo.file-info`,
`repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.create-branch`.
Presence, expected definition/classification, current repository where needed,
allowed permission, and capability preconditions are all required. Visibility,
effect class, permission, frontend, and provider metadata are insufficient.

## Input / confirmation

No arbitrary JSON console. Backend-owned typed bounded descriptors govern
input; Tool parsing/policy remains final. Read-only submission needs no second
confirmation. `repo.create-branch` is prepare -> sanitized review -> explicit
confirm -> revalidate -> dispatch, with only its bounded `name` user-editable.

## Ticket binding

Effectful work uses an opaque RAH-generated in-memory, nonserializable,
process-local, bounded-lifetime single-use ticket bound to Tool, full expected
definition, reconstructed input, review, composition/registry, permission
policy, repository/currentness generations, Effective Authority, and relevant
capability authority. Stale tickets fail closed with no automatic re-prepare.

## Concurrency

At most one prepared/in-progress host invocation; no model-turn overlap,
including read-only host work. Existing Tool/capability leases and provider
lifecycle locks remain authoritative.

## Provenance / lifecycle

Desktop-private source is `HostExplicit`; no `AgentEvent` schema change or fake
model `ToolRequested`/`ToolStarted`/`ToolFinished`. Dispatch states remain
separate from structured Tool results and reuse existing result/currentness/
review semantics.

## Cancellation / uncertainty

Cancellation is supported only before Tool start (`cancelled_before_start`).
After start, UI dismissal does not abort owned execution. Lost terminal result
after possible effect is conservative `possible_effect_unknown`; no retry,
replay, compensation, or rollback claim.

## Conversation / persistence

No chat message, model-context injection, automatic agent continuation, or
persisted ticket/authority/in-flight work. Restart freshly composes and
observes state, without resuming or claiming effects absent.

## External and repo.commit deferrals

External MCP and Process Plugin Tools are deferred; provider metadata cannot
self-declare eligibility. `repo.commit` remains deferred because reviewed
commit authorization is separate and one-shot. Worktree authoring Tools remain
deferred pending capability-specific typed review contracts. Echo and fixture
Tools are not product host-invokable.

## Security nonclaims

No capability amplification, model-text trigger, frontend authority, provider
self-eligibility, generic JSON, generic filesystem/Git/process/network
authority, replay, rollback, OS sandbox, or network isolation claim.

## Files changed

- `docs/adr/0021-explicit-host-tool-invocation-dispatch-boundary.md`
- `docs/plans/2026-09-07-adr-0021-explicit-host-tool-invocation-dispatch-boundary.md`

## Validation

Pending sequential `cargo fmt --check`, `cargo check --workspace`, `git diff
--check`, and `cargo metadata --no-deps --format-version 1`; verify 13 packages,
all `0.19.0`, edition 2024, and no `Cargo.toml`/`Cargo.lock` diff.

## Commit

Pending `docs: define explicit host invocation dispatch boundary` after scope
audit confirms exactly these two documentation files.

## Exact-head CI

Pending: after push, require completed successful `push` CI on `master` for the
exact Task 236 commit.

## Next task

Task 237 — Minimal Authorized Tool Dispatch Foundation: not started.
