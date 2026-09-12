# Task 316 — Inert Multi-Repository Membership and Explicit Admission Foundation

Status: implementation complete; closure evidence follows the production and documentation commits

## Authoritative checkpoint

- Current master: `6443525ad2ffdc5e4407e2eb1b8f214acbdae4a8`
- Direct parent: `8ee429839ef32b901625a5d022a86c64b6888272`
- Task 315 exact-head CI: `34684342066` (PASS)
- Task 315 verdict: Verdict A — nested-boundary conformant; clear for the multi-repository membership foundation
- Accepted architecture: ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary
- Release baseline: RAH v0.25.0, tag `v0.25.0`, immutable source `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`

## Scope and constraints

Task 316 adds process-local, host-owned inert membership for explicitly admitted
repositories. It preserves one optional active `DesktopRepository`, one active
repository-bound ToolRegistry/composition, the existing global HostExplicit
coordinator, and the existing reviewed Commit/workflow paths. Membership is
descriptive and is never a workspace authority or Tool dispatch route.

No persistence, frontend multi-repository UX, repository switching command,
provider activation, cross-repository operation, Tool schema change, HostExplicit
eligibility change, or ADR 0027 rewrite is in scope.

## State model

`DesktopAppState` owns a process-local membership catalog containing a private
workspace epoch, membership generation, fresh opaque member-ID allocator,
bounded inert member records, and an optional active member ID. A member record
contains private canonical root/Git-bound identity evidence and a display path;
it contains no `ToolRegistry`, `DesktopRepository`, mutation authority,
`RepositoryCommitControl`, workflow, HostExplicit state, provider, or runtime.

The existing `repository_generation` remains the active repository/current
composition generation. Adding an inactive member increments only membership
generation. Activation increments active repository generation and publishes the
active member handle with the fresh active `DesktopRepository`.

## Identity and admission

The narrow repository admission identity binds a canonical existing root, root
filesystem identity, supported directory `.git` identity, canonical Git
executable identity, ordinary non-bare worktree form, and process-local
admission currentness. It rejects links/reparse ambiguity and `.git` file or
linked-worktree forms. It supports revalidation, same-repository comparison, and
parent/child relation classification without exposing raw filesystem identity.

Admission performs lifecycle checks, fresh Git/identity validation, temporary
current capability construction, duplicate/alias comparison, nested conflict
comparison, a generation/currentness recheck, and one atomic inert publication.
Failed admission leaves membership, active state, repository generation,
workflow, Commit, and conversation state unchanged.

## Activation and invalidation

The internal host activation helper validates a catalog handle, revalidates its
identity and current Git binding, freshly constructs the active repository,
rejects model turns and running HostExplicit work, clears prepared HostExplicit
state, revokes the old Commit capability, resets workflow/action/review state,
starts a fresh repository conversation namespace, and publishes the new active
member/repository generation together. Connected/connecting/disconnecting
states remain blocked.

## Deterministic test matrix

Focused Desktop tests cover startup emptiness and non-activation, two distinct
fresh repositories, inactive membership currentness, duplicate and alias
rejection, both nested admission orders, unsupported `.git` forms, stale root
and `.git` activation rejection, fresh activation and generation behavior,
active-only authority, workflow/Commit/HostExplicit invalidation, lifecycle
blocking, conversation separation, no persistence, privacy-safe state, and the
unchanged 11-name HostExplicit set. Existing Task 315 nested-boundary suites
remain a required regression gate.

## Validation and closure

Required validation is `cargo fmt --check`, workspace check/test/clippy,
`git diff --check`, `cargo metadata --no-deps --format-version 1`, focused
Desktop/identity tests, nested-boundary regressions, and the established
Desktop release/permission checks where applicable. No connected destructive
Codex test is required. The implementation must be committed, pushed to
`master`, and closed only after exact-head CI passes with a clean worktree.

Production commit subject: `feat: add multi-repository membership foundation`.

## Implementation validation before production commit

The deterministic implementation gates passed before the production commit:

- `cargo fmt --all -- --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace` — PASS; the new Desktop matrix passed, Desktop
  reported 220 passed and 10 ignored, and all workspace crates and integration
  suites completed without failure.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS; 13 packages, one
  version `0.25.0`, edition `2024`, and no manifest or lockfile drift.
- `git diff --check` — PASS.
- Focused identity test — PASS (1 test).
- Focused Desktop Task 316 matrix — PASS (1 test).
- `cargo test -p rah-desktop` — PASS (220 passed, 10 ignored).
- `cargo test -p rah-tools` — PASS (285 unit tests plus all integration tests).
- `cargo test -p rah-tools --test repository_nested_boundary` — PASS (1 test).
- `cargo build -p rah-desktop --release` — PASS.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.

The production commit and final exact-head CI evidence are recorded in the
closure follow-up after publication. No connected destructive Codex test was
run.
