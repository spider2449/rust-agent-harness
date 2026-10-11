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

The production commit and final exact-head CI evidence are recorded below. No
connected destructive Codex test was run.

## Closure report

**Outcome A — MEMBERSHIP FOUNDATION CLOSED.**

The implementation is process-local and deterministic. The direct production
commit is `ad56f0c85bae5a41175e85d37e534f57838d8825`, with checkpoint
`6443525ad2ffdc5e4407e2eb1b8f214acbdae4a8` as its parent. Two narrow
cross-platform test corrections followed: `742739cbf9711030c684b813ed4b612dc8143ebe`
uses a regular current executable in the identity fixture, and
`185db1d24cbcab81823ac76a041df53a87c76f3b` removes conditional return forms
rejected by Unix clippy. The final code exact-head CI is run `34699519435`,
which passed formatting, workspace check, workspace test, and workspace lint
for `185db1d24cbcab81823ac76a041df53a87c76f3b`.

The state type is `WorkspaceMembershipState` in
`crates/rah-desktop/src/repository_membership.rs`. It owns a process-local
workspace epoch, membership generation, monotonic member allocator, bounded
inert member map, and optional active member handle. `RepositoryMemberId`
contains only the private process epoch and ordinal. It is host-generated,
non-persistent, opaque to the frontend/model/provider, and not an authority
or authorization token.

The narrow `rah-tools` helper `RepositoryAdmissionIdentity` was required. It
privately binds canonical root, root filesystem identity, supported directory
`.git` identity, canonical Git executable identity, and fresh capture
currentness. It revalidates those bindings, rejects symbolic-link/reparse
ambiguity and linked-worktree `.git` files, and compares captured roots as
same, nested, or distinct without exposing raw identity values or generic
filesystem authority.

Admission is host-owned: fresh Git and identity validation, temporary current
repository capability construction, identity revalidation, duplicate/alias
comparison, nested comparison, and inert publication under the coordination
lock. Duplicate identity, canonical alias, case-equivalent Windows spelling,
or shared root/`.git` identity returns `RepositoryAlreadyMember` without
focusing or activating the existing member. A canonical parent/child relation
in either direction returns `RepositoryNestedMembershipConflict`. No
automatic repository scan occurs. Legacy `choose_repository` admits and then
freshly activates the new member; activation failure removes that just-created
inert member so the command remains all-or-nothing.

Activation accepts only a catalog member handle, revalidates its identity and
Git binding, freshly constructs `DesktopRepository` and its current authority
objects, checks idle/disconnected lifecycle state, clears prepared
HostExplicit state, revokes old Commit context, resets workflow/action/review
state, starts the existing fresh conversation namespace, and publishes the
active member with the active repository. Model turns and running HostExplicit
effects reject activation; connecting, connected, and disconnecting states
remain rejected. No inactive member retains a ToolRegistry, DesktopToolComposition,
DesktopRepository, mutation authority, CommitControl, workflow, HostExplicit
preparation, provider Tool, or runtime connection.

Membership generation changes for inert admission/removal only. The existing
`repository_generation` remains the active repository/current composition
generation: inactive B admission does not change active A, while each fresh
activation increments it. The current active member handle and current
authority-bearing repository are published through the host coordination path;
there is no per-member executable composition.

Active change withdraws old Commit authorization, clears Stage/Unstage action
selectors and review state, invalidates prepared HostExplicit state, and
requires fresh review for the new active repository. The global
`HostInvocationCoordinator` remains authoritative. Conversation state starts
fresh on active change and is never migrated or resumed automatically.
Membership is absent from persistence and startup creates zero members and no
active repository. Effective Authority and ToolRegistry remain active-only;
there is no workspace union, repository ID in ToolInput, model-selected
switching, provider activation, or cross-repository operation. HostExplicit
eligibility remains exactly 11 names.

The focused matrix is one Desktop test plus one identity test. The Desktop
crate passed 220 tests with 10 ignored; `rah-tools` passed 285 unit tests and
all integration suites; the nested-boundary regression passed one test. The
full workspace test passed locally and in final exact-head CI. Package
metadata remains 13 packages, version `0.25.0`, edition `2024`; there is no
dependency or `Cargo.lock` drift. Frontend multi-repository UX, persistence,
removal UX, explicit switching, full product tabs, and live certification are
deferred to later tasks.

The v0.25.0 immutable identity remains unchanged: source
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, annotated tag `v0.25.0`, and tag
object `ea3c31aaf5190b632d7ef86387f7aff6004ae664`. The next task is Task 317,
active repository switching and active-only composition integration.
