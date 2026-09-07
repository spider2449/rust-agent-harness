# Task 247 — ADR 0022: HostExplicit Reviewed Worktree Authoring Boundary

## Status

Documentation-only ADR and plan prepared; awaiting commit and exact-head CI.

## Starting checkpoint

Resolved dynamically with `git rev-parse --show-toplevel`.

- repository: `spider2449/rust-agent-harness`;
- `HEAD == origin/master == 4046fde3e431fa0986d2bbfdadac8a9f1b0937d6`;
- Task 246 CI: `34096749687 PASS`;
- worktree: clean;
- workspace: 13 packages, all `0.20.0`, edition 2024;
- dependency drift: none.

No fixed drive, checkout path, user, temporary path, home directory, or
machine-specific value is part of this plan.

## Task 246 decision

Task 246 selected A1 HostExplicit Reviewed Single-File Patch Authoring and
Option B: create ADR 0022 after capability-specific research. Its authoritative
choices are H1 human input, P2/O2 shared non-effectful preparation in
`rah-tools`, R4 exact changed-range plus escaped old/new review, N2 no-op
refusal without a ticket, ticket-only confirmation, D2 preflight and
execution revalidation, and conservative possible-effect handling.

Task 246 semantics for review, bounds, escaping, ticket bindings, currentness,
result classification, failure, refresh, and reviewed-commit invalidation are
preserved without redesign.

## ADR scope

Create ADR 0022 with status Accepted, date 2026-09-07, and title:

    ADR 0022 — HostExplicit Reviewed Worktree Authoring Boundary

The ADR records that:

1. ADR 0012 remains the sole underlying `repo.patch` worktree-content mutation
   authority.
2. ADR 0021 remains the general HostExplicit dispatch/currentness/D2/provenance
   boundary.
3. ADR 0022 adds only a durable reviewed human workflow for H1 `repo.patch` in
   the first v0.21 scope.
4. `repo.create-file`, `repo.edit-files`, `repo.delete-file`,
   `repo.rename-file`, `repo.create-directory`, `repo.commit`, and external
   provider Tools remain disabled.
5. Preparation uses the shared non-effectful P2/O2
   `RepositoryPatchPreparer`; it is not a Tool, cannot mutate, must not call
   `Tool::execute`, and does not introduce a generic registry downcast or
   mutation-preview API.
6. The host derives all hashes, lengths, canonical ToolInput, identities,
   definition, permission, composition/currentness, and review bindings from
   the human’s path and literal text.
7. Prepare is zero-effect and requires Idle, connected-current state, backend
   eligibility, D2 preflight, shared preparation, exact review, and an opaque
   ticket. N2 creates no ticket for a known no-op.
8. Review is R4 display-only content: complete changed range plus complete
   safely escaped old and replacement text, with no hidden changed material and
   no executable diff semantics.
9. Tickets are opaque, RAH-generated, in-memory, process-local, single-use,
   non-persistent, and five-minute TTL objects. Confirm receives only the
   ticket ID and never refreshes or reprepares automatically.
10. Confirm follows ticket validation, host currentness/eligibility,
    `authorize_tool_dispatch`, HostExplicit Started,
    `authorized_tool_dispatch`, ToolRegistry, existing `repo.patch`, and ADR
    0012 policy. Raw registry execution is insufficient authorization.
11. ADR 0021 concurrency and HostExplicit-private provenance are reused. No
    model-turn overlap, fake AgentEvents, conversation injection, or active
    post-start abort is added.
12. Only the intended contents of one existing tracked file may change. No
    index, HEAD, branch, ref, history, other file, provider/profile, or
    conversation mutation is authorized. Started effectful patch execution
    conservatively clears reviewed commit authorization; Prepare does not arm
    it.
13. The existing four-status `repo.patch` taxonomy and strict ToolOutput
    validation are reused. Malformed, lost, contradictory, or errored
    post-start handling is possible-effect/uncertain. No retry, replay,
    rollback, restore, compensation, or automatic second confirmation is
    added.
14. Full source content remains bounded to process-local prepared state and
    the local review; it is not persisted to conversation SQLite or generic
    activity history and is not emitted to diagnostic/live-evidence logs.
15. The ADR explicitly preserves all requested security nonclaims, including
    no generic filesystem, arbitrary full-file, multi-file, diff, Git, shell,
    process, retry/replay/rollback, OS-sandbox, network-isolation, or race-free
    TOCTOU authority.

ADR 0021 is not rewritten: v0.20 remains historical, and authoring was
deferred there pending the capability-specific review and preimage research
completed by Task 246.

## No new underlying authority

This task changes architecture documentation only. It does not change the
existing `repo.patch` policy, Tool schema, ToolRegistry, public permissions,
composition, frontend, Desktop, Rust implementation, tests, Cargo manifests,
dependencies, README, CHANGELOG, release documentation, workflow, or scripts.

## Files changed

Exactly these two files are in scope:

- `docs/adr/0022-hostexplicit-reviewed-worktree-authoring-boundary.md`;
- `docs/plans/2026-09-07-adr-0022-hostexplicit-reviewed-worktree-authoring-boundary.md`.

## Validation

Run sequentially:

    cargo fmt --check
    cargo check --workspace
    git diff --check
    cargo metadata --no-deps --format-version 1

Require metadata to report 13 packages, all `0.20.0`, edition 2024. Also run:

    git diff -- Cargo.toml Cargo.lock

and require empty output. Before commit, inspect `git status --short`,
`git diff --stat`, and `git diff --check`; the only changes must be the two
documentation files above.

## Commit

Commit exactly:

    docs: define reviewed HostExplicit authoring boundary

Before push, run `git fetch origin` and require:

    origin/master == 4046fde3e431fa0986d2bbfdadac8a9f1b0937d6

If `origin/master` moves unexpectedly, stop and do not silently rebase or
change chronology. Push `master` normally.

## Exact-head CI requirement

Task 247 is complete only after master push CI for the exact Task 247 commit
has:

- `head_sha` equal to the Task 247 commit;
- `status == completed`; and
- `conclusion == success`.

An absent, queued, in-progress, or different-head run is not success.

## Next task

Task 248 — Shared Non-Effectful `repo.patch` Preparation Foundation.

Do not start Task 248 automatically.
