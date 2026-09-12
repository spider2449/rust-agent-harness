# Task 310 — Workspace/Repository Identity and Authority-Composition ADR Decision

Status: Decision A — ACCEPTED

## Scope

Task 310 is an ADR/decision-only task. It changes no Rust source, frontend,
Tauri permissions, Tool schema, HostExplicit eligibility, ToolRegistry or
Effective Authority implementation, repository selection behavior,
conversation persistence behavior, Commit or Stage/Unstage behavior, provider
composition, Trusted Profile behavior, dependency, Cargo version, CI workflow,
release/tag/publication state, or production authority.

The exact intended changed scope is:

```text
docs/adr/0027-workspace-repository-identity-authority-composition.md
docs/plans/2026-09-12-task-310-workspace-repository-authority-adr-decision.md
```

## Authoritative starting checkpoint

| Item | Value |
| --- | --- |
| Branch | `master` |
| Current master | `7971d253dd6f89717b682773ded06235c5daf39a` |
| Direct parent | `ad6ae067f7248c50873a6d4b08d6c60db33b4e50` |
| Task 309 exact-head CI | `34667437233` — PASS |
| Task 309 outcome | Outcome A — MULTI-REPOSITORY BOUNDARY VIABLE |
| Selected architecture | Option A — one active repository with fresh active-only composition |
| Current release | RAH v0.25.0 — RELEASED |
| Immutable release source | `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` |
| Annotated tag object | `ea3c31aaf5190b632d7ef86387f7aff6004ae664` |
| GitHub Release | `387406579` |
| Workspace baseline | 13 packages, version `0.25.0`, Rust edition 2024 |
| Production HostExplicit set | exactly 11 names, unchanged |

Before editing, the worktree was clean, the checkout was `master`, and
`origin` was the GitHub remote at the supplied current master. ADR 0027 and
this Task 310 plan did not exist, and ADR numbering was therefore available.

## Evidence reviewed

The independent gate reviewed:

- Task 308, `docs/plans/2026-09-12-v0.26-scope-and-authority-roadmap.md`;
- Task 309, `docs/plans/2026-09-12-v0.26-multi-repository-identity-authority-research.md`;
- `README.md`, `docs/ARCHITECTURE.md`,
  `docs/ARCHITECTURE_GUARDRAILS.md`, and `docs/SECURITY.md`;
- ADR 0003, ADR 0011, ADR 0016, ADR 0018, ADR 0021, and ADRs 0022–0026; and
- current production code in `rah-desktop`, `rah-tools`, and `rah-sandbox`.

Current code confirms the v0.25 single-repository boundary: one
`DesktopRepository`, one scalar repository generation, one repository-bound
workflow/Commit state, one current first-party registry composition, one
Effective Authority repository binding, one global HostExplicit coordinator
slot, and model-turn/currentness capture tied to the connected repository.
The HostExplicit tests and descriptors confirm the exact 11-name production
set.

## Independent decision gate

Task 308 recommended the v0.26 multi-repository workspace/repository selection
direction but did not authorize implementation or ADR acceptance. Task 309
selected Option A and found the boundary viable only when workspace membership
is descriptive and every executable/mutating operation remains bound to one
repository identity.

The accepted ADR preserves that boundary. No contradiction was found with the
accepted security contracts. ADR 0003, ADR 0011, ADR 0016, ADR 0018, ADR 0021,
and ADRs 0022–0026 remain authoritative and are wrapped rather than amended.

The material implementation issue is nested-repository enforcement. Task 309
correctly recorded that the reviewed rename path checks nested `.git`, but
`fs.read` and other repository-bound paths do not yet establish uniform
enforcement. This is not hidden or treated as proof of current conformance.
The accepted ADR makes it a later conformance gate and recommends Task 311
begin with a focused nested-boundary audit/research task. This does not broaden
`WorkspacePolicy`, add a workspace permission, or authorize implementation in
Task 310.

## Accepted decision

ADR 0027 establishes:

- workspace as descriptive user organization, not a filesystem root or
  permission;
- fresh host-owned repository identity as the scope of repository authority;
- explicit host/human admission, with duplicate/alias-equivalent membership
  rejected as `already_member`;
- rejection of nested repository co-membership and independent nested `.git`
  boundary failure for every repository-bound path capability;
- zero or one active repository and fresh active-only composition;
- no workspace-wide ToolRegistry or union Effective Authority;
- repository-specific generations/currentness and no authority migration from
  unrelated members;
- switch invalidation of prepared HostExplicit state, reviewed Commit state,
  and Stage/Unstage state;
- immutable repository binding for each model turn;
- separate per-repository descriptive conversation context, as selected by
  Task 309, with optional workspace overview;
- inert descriptive persistence and no authority restoration after restart;
- sanitized provider/model/activity state and sentinel-value privacy tests;
- no cross-repository effects or parallel multi-repository mutation; and
- no change to the exact 11-name HostExplicit eligibility set.

The ADR also preserves running-effect uncertainty semantics: switching or
removal does not migrate, roll back, or replay an already-started effect.

## Alternatives

Rejected alternatives are recorded in ADR 0027:

1. Keep the current single-repository replacement model: safe, but insufficient
   for the v0.26 workspace UX.
2. Retain executable compositions per inactive repository: unnecessary
   lifecycle, currentness, ticket, provider, and privacy complexity.
3. Form a workspace-wide union registry: authority routing through model Tool
   names/inputs, collision risk, and a weaker active boundary.
4. Treat the workspace root as filesystem authority: forbidden generic
   authority.
5. Auto-discover nested repositories: repository contents cannot grant
   authority.
6. Persist executable authority: conflicts with process-local currentness and
   nonpersistent HostExplicit semantics.

## Validation and delivery record

No live or destructive test was run. No implementation was performed. The
required validation was run sequentially:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS |

Decision commit SHA: `f3de3ebb96f17d56195ff72ac74591e08d2c7119`

Direct parent: `7971d253dd6f89717b682773ded06235c5daf39a`

Decision commit exact-head CI: `34668338465` — PASS

The final delivery must verify `HEAD == origin/master`, a clean worktree, the
two-file scope, and unchanged v0.25.0 immutable release identity.

## Next task

Task 311 should be narrowly scoped to the repository nested-boundary
conformance audit/research across existing path capabilities. If the audit
finds corrections are needed, correct and test those boundaries before
workspace membership/admission implementation. If it proves sufficient
conformance, proceed to inert membership and explicit host admission. No
general multi-repository implementation is authorized by this plan.

## Closure conditions

Decision A is complete only when the accepted ADR and this plan are the exact
changed files, the final commit is pushed to `origin/master`, exact-head CI
passes for that commit, and the worktree is clean. The immutable v0.25.0
source, annotated tag object, GitHub Release, version, dependencies, CI
workflow, and HostExplicit eligibility remain unchanged.
