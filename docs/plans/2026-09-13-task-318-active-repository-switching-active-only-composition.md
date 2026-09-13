# Task 318 — Active Repository Switching and Active-Only Composition

Status: implementation complete; publication/CI closure pending

## Authoritative checkpoint

- Current master: `b8065fbaee3ad5f6b210940b85391b1372f07472`
- Direct parent: `ad1e9a4361516c7ef984d0ab7eefa172d336cb15`
- Task 317 production correction: `ad1e9a4361516c7ef984d0ab7eefa172d336cb15`
- Task 317 docs closure: `b8065fbaee3ad5f6b210940b85391b1372f07472`
- Task 317 exact-head CI: `34728441103` (PASS)
- Task 316: CLOSED — MEMBERSHIP FOUNDATION CLOSED
- Accepted architecture: ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary
- Current release: RAH v0.25.0, immutable source `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`

## Scope and authority boundary

Task 318 exposes explicit human selection among process-local admitted
members. Membership remains descriptive and inert. There remains exactly one
authority-bearing path:

```text
opaque member selector
  -> strict host lookup
  -> activate_admitted_member
  -> Task 317 currentness/publication gate
  -> fresh active-only repository state
```

The selector is a bounded, process-local string derived from the private
`RepositoryMemberId` fields. It contains no path or filesystem identity and is
not authorization; host lookup and identity revalidation remain authoritative.

The presentation contains bounded member IDs, user-facing display names,
active state, and the membership generation. It does not expose admission
identity, Git identity, tool registries, permissions, Commit authorization,
HostExplicit tickets, provider credentials, or runtime state.

## Implementation plan

1. Add strict selector encoding/decoding and a read-only membership presentation
   command.
2. Add one narrow `activate_repository_member` Tauri command and route it to
   the existing activation transaction. Selecting the active member is a
   bounded no-op with no generation, conversation, workflow, Commit, Stage,
   Unstage, or HostExplicit changes.
3. Add the minimal frontend member selector and explicit switch action. Keep
   controls advisory; backend lifecycle checks remain authoritative.
4. Add deterministic Rust, frontend, and Tauri permission tests for selector
   privacy, inert membership, lifecycle rejection, active-only composition,
   invalidation, persistence, and existing Task 315/317 regressions.
5. Run focused and workspace validation, desktop release build, inspect the
   complete staged diff, commit the coherent production/docs changes, push
   `master`, and verify exact-head CI.

## Explicit deferrals

No workspace membership persistence, member removal, tabs/dashboard, rename,
reorder, cross-repository operation, model/provider-selected switching,
connected runtime migration, per-member executable composition, authority
union, workspace-wide conversation, automatic transcript resume, or live
multi-repository certification is in scope.

## Implemented contract

- Membership presentation: `{ members, activeMemberId, membershipGeneration }`.
- Member presentation: bounded opaque `memberId`, bounded basename
  `displayName`, `active`, and descriptive `availability` (`active` or
  `inactive`). Private admission identity, canonical root, Git identity,
  registries, permissions, Commit state, tickets, provider state, and runtime
  state remain absent.
- Selector: strict process-local `m<workspace_epoch>-<ordinal>` encoding of
  `RepositoryMemberId`; decimal components are nonzero, canonical, bounded,
  and contain no native path or filesystem evidence. It is descriptive routing
  only, never authorization.
- Read command: `repository_membership`; it only reads the process-local
  catalog and does not resolve Git, revalidate members, construct authorities,
  compose tools, activate providers, or mutate generations/conversation.
- Switch command: `activate_repository_member({ memberId })`; malformed and
  unknown selectors fail closed, and valid selectors route to the existing
  `activate_admitted_member` Task 317 transaction. The active selector returns
  `already_active` without changing repository generation, conversation,
  workflow, Commit, Stage/Unstage, or HostExplicit state.
- Successful A→B activation publishes exactly one current B repository,
  increments repository generation once, starts a fresh conversation, clears
  A workflow/review/Commit state, invalidates prepared HostExplicit state, and
  leaves inactive A descriptive and inert. Fresh composition receives B only.
- Frontend is a small repository-management selector plus explicit switch
  button. It sends only `memberId`; controls are advisory and disabled for
  chat/model and Connecting/Connected/Disconnecting states. No model Tool,
  provider metadata, path input, generic dispatcher, removal, or persistence
  was added.

## Deterministic evidence

The new Rust test `task_318_membership_presentation_and_selector_route_are_active_only`
uses two distinct disposable Git repositories and verifies empty startup,
active/inert listing, bounded distinct selectors, privacy, malformed and
forged selector rejection, already-active no-op preservation, A→B result,
active B root/generation, fresh B composition, workflow/Commit/ticket
invalidation, and fresh conversation state. Existing Task 316/317 tests retain
the stale identity, `.git` replacement, lifecycle, nested membership, restart,
provider, and exact 11 HostExplicit checks.

Executed validation before publication:

- `cargo fmt --check` — PASS
- `cargo check --workspace` — PASS
- `cargo test --workspace` — PASS; Desktop 229 passed/10 ignored, workspace
  suites passed
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS
- `git diff --check` — PASS
- `cargo metadata --no-deps --format-version 1` — PASS; 13 packages, version
  0.25.0, edition 2024
- `cargo build -p rah-desktop --release` — PASS
- `node --check crates/rah-desktop/frontend/status.js` — PASS
- membership frontend, Effective Authority frontend, and Tauri permission
  tests — PASS

No Cargo dependency or lockfile drift. No connected destructive live test or
multi-repository live certification was run.

The deterministic matrix is covered by the new Task 318 test plus retained
Task 316/317 coverage: 1 empty presentation; 2 A active; 3 inert B; 4
privacy; 5 distinct bounded selectors; 6 malformed selector; 7 forged
selector; 8 already-active selection; 9-11 no generation/conversation/
authority churn; 12-15 A→B/active root/generation; 16 prepared ticket
invalidation; 17-21 HostExplicit/model/connection lifecycle blocking; 22-23
stale and replaced `.git` fail-closed; 24 Commit withdrawal; 25-26
Stage/Unstage and fresh B workflow; 27-29 fresh conversation and explicit-only
resume; 30 active-only Effective Authority; 31 fresh B composition; 32 inert
inactive authority; 33 duplicate admission does not switch; 34 nested rule;
35-36 restart and no persistence; 37 provider isolation; 38 exact 11
HostExplicit names; 39 Task 317 race suite; 40 Task 315 nested-boundary suite.

## Commits and exact-head closure

- Production commit: `4d975a99636fd8567d5811b69398eb0a8e166ce9`, subject
  `feat: add explicit repository switching`.
- Production parent chain: `4d975a9` → `b8065fb` → `ad1e9a4` → `e741e14`.
- Production exact-head CI: run `34731127097`, PASS, for
  `4d975a99636fd8567d5811b69398eb0a8e166ce9`.
- Production changed files: `crates/rah-desktop/build.rs`,
  `crates/rah-desktop/capabilities/default.json`,
  `crates/rah-desktop/frontend/index.html`,
  `crates/rah-desktop/frontend/repository_membership_test.js`,
  `crates/rah-desktop/frontend/status.js`,
  `crates/rah-desktop/permissions/autogenerated/activate_repository_member.toml`,
  `crates/rah-desktop/permissions/autogenerated/repository_membership.toml`,
  `crates/rah-desktop/src/main.rs`,
  `crates/rah-desktop/src/repository_membership.rs`, and
  `crates/rah-desktop/tauri_permission_test.js`.
- Docs closure commit: `c07c6cee7686d12dd1bf32792fcdab0bbd3711f1`, subject
  `docs: close Task 318 evidence`.
- Docs closure exact-head CI: run `34731217120`, PASS, for
  `c07c6cee7686d12dd1bf32792fcdab0bbd3711f1`.
- Final remote alignment: `HEAD == origin/master ==
  c07c6cee7686d12dd1bf32792fcdab0bbd3711f1`; the worktree is clean.
- Immutable v0.25 recheck: annotated tag object
  `ea3c31aaf5190b632d7ef86387f7aff6004ae664` resolves to source commit
  `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`; GitHub Release `387406579`
  remains tagged `v0.25.0`.
- Final changed-file scope is the ten production/test/permission files listed
  above plus this required plan file; no Cargo or lockfile changes exist.

Closure outcome must be exactly one of:

- Outcome A — ACTIVE REPOSITORY SWITCHING CLOSED
- Outcome B — BLOCKED
