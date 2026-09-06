# Task 228 — Desktop / Effective Authority Local Branch Creation Integration

## Result

Status: IMPLEMENTED — AWAITING EXACT-HEAD CI (CORRECTIVE COMMIT)

The initial Task 228 implementation commit was
`d19512f27b0aa00a5fbe5f2279033728dbad645e`; its exact-head CI run
`34030144127` passed. An independent source audit found two semantic gaps in
the initial implementation: branch output safety was classified from the
status string alone, and cleanup did not treat a started first-party
`repo.create-branch` call without `ToolFinished` as an unresolved effect.
Task 228 therefore remains open while this bounded corrective commit closes
those gaps.

Desktop now composes the existing ADR 0020 local branch creation authority only
for a host-selected repository, conditionally publishes its first-party Tool,
and presents it as a separate sanitized Effective Authority category.

## Starting checkpoint

The corrective work starts from
`d19512f27b0aa00a5fbe5f2279033728dbad645e`, with `HEAD == origin/master`, a
clean worktree, and the dynamically resolved origin remote. The repository
remains on the existing 0.18.0 / edition 2024 / 13-package baseline.

## ADR 0020 authority chain

The existing public `RepositoryBranchCreationAuthority`,
`RepositoryBranchCreationTool`, and `REPOSITORY_CREATE_BRANCH_TOOL_NAME` are
used unchanged:

```text
host-selected repository
 -> RepositoryBranchCreationAuthority
 -> DesktopRepository
 -> RepositoryBranchCreationTool
 -> Desktop ToolRegistry
 -> Generic Codex Tool Bridge
 -> sanitized Effective Authority
```

No branch semantics, generic Git API, profile capability, or provider path was
added.

## Desktop authority ownership

`DesktopRepository` carries an optional host-created branch authority. Trusted
composition rejects an authority whose Git executable or canonical repository
does not match the selected resources.

## Repository selection

Selection attempts branch-authority construction after host Git selection.
Construction failure logs a bounded host-side diagnostic and leaves the
optional capability absent while preserving otherwise valid repository
selection.

## Registry composition

The Desktop registry registers `repo.create-branch` only from the stored
authority via `RepositoryBranchCreationTool::from_authority`. No repository or
authority means no Tool and no implicit/default registration occurs.

## Effective Authority classification

`repo.create-branch` is classified as `repository_mutation` and the closed
`repository_local_branch_creation` authority category. It is repository-bound,
repository-host sourced, labeled `desktop_repository`, and retains the Tool
definition's `execute` permission.

## Unavailable-state behavior

Without a repository, the known capability is `configured_unavailable` with
`repository_required`. With a selected repository but no branch authority, it
uses `authority_not_granted`.

## Runtime advertisement

The existing Generic Codex Tool Bridge consumes the resulting registry. The
Tool is marked advertised only under the existing connected/current publication
rule; configuration alone does not advertise it.

## Reviewed commit preservation

Verified and safe branch outcomes do not invalidate reviewed commit
authorization or request a repository workflow refresh.

## Fully uncertain outcome handling

Exact `uncertain` and malformed/unrecognized branch output are handled
conservatively: selected repositories invalidate review and request the
existing bounded first-party refresh. No replay, rollback, or delete is added.

The corrective implementation makes this classification a closed parser. It
requires exactly one JSON object with the exact status-specific key set,
consistent `is_error` and `uncertain` values, non-empty branch names, and
full 40- or 64-character hexadecimal OIDs. The exact `uncertain` result is
not proven safe. Malformed recognized results, extra fields, non-JSON content,
and multiple content values are all uncertain.

Turn cleanup now treats a started `repo.create-branch` call as an unresolved
repository effect even when it is first-party and has no `ToolFinished` event.
Requested-but-not-started branch calls remain non-effectful. With a selected
repository cleanup invalidates review and requests the bounded refresh, then
clears activity; without one it clears activity without manufacturing refresh
state. The correction does not change repository, model, profile, connection,
or conversation generations and does not replay, reconnect, or roll back.

## Repository generation

Branch Tool completion does not increment `repository_generation`, regardless of
outcome.

## Connection currentness

Create-only branch ref effects do not disconnect, reconnect, recompose, or
re-publish the runtime. The existing currentness tuple remains authoritative.

## Conversation/provider non-change

Branch creation does not change conversation identity or repository namespace,
model/profile/connection generations, provider activation, or provider
inventory.

## Frontend presentation

The frontend maps `repository_local_branch_creation` to `Local branch creation`
and renders the backend category through the existing authority renderer. It
does not infer authority from Tool names or add branch controls.

## Startup inertness

Startup still constructs inert Desktop state only. Git resolution, branch
authority construction, repository Tool composition, provider activation, and
Codex startup remain explicit-action paths.

## Deterministic tests

Focused Desktop tests cover authority ownership/resource mismatch, conditional
registry registration and dispatch, exact committed HEAD targeting without
switching, Effective Authority classification and unavailable reasons, current
runtime advertisement, and safe versus uncertain branch activity semantics.

## Security / redaction

Only the logical branch name reaches the existing Tool input. Repository/Git
resources remain host-bound. Effective Authority and live evidence contain no
path, OID, argv, environment, hook path, or stderr; no Trusted Profile,
provider metadata, frontend state, or Execute permission can manufacture the
authority.

## Validation

Validation completed before CI:

- `cargo fmt --check` — PASS
- `cargo check --workspace` — PASS
- `cargo check -p rah-desktop --tests --target x86_64-pc-windows-msvc` — PASS
- `cargo test -p rah-tools --lib repository_branch_create -- --nocapture` — 29 PASS
- `cargo test -p rah-desktop` — 177 PASS, 2 ignored
- `cargo test --workspace` — PASS
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS
- `git diff --check` — PASS
- `cargo metadata --no-deps --format-version 1` — PASS; 13 packages, version 0.18.0, edition 2024
- `node --check crates/rah-desktop/frontend/status.js` — PASS
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS
- `cargo build -p rah-desktop --release` — PASS

The exact requested broad `cargo test -p rah-tools repository_branch_create
-- --nocapture` command was also attempted, but its Windows MSVC linker failed
while linking unrelated test binaries with `link.exe` `LNK1000`/`0xc0000005`.
The branch module suite passed through the focused `--lib` fallback and the
full workspace suite.

## Commit

Initial commit: `d19512f27b0aa00a5fbe5f2279033728dbad645e`.
Corrective commit: pending.

## Exact-head CI

The initial commit's exact-head CI `34030144127` passed. The corrective commit
must be pushed and verified with a new exact-head CI success before this plan
can be marked complete.

## Deferred work

Windows live Desktop validation remains deferred to Task 229. No Trusted
Profile/provider authority, Codex-specific branch API, branch UI, checkout, or
generic Git work is included.

## Next task

Task 229 — Windows Live Desktop repo.create-branch Validation. It is not
started automatically by Task 228.
