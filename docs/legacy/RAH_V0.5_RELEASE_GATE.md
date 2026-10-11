# RAH v0.5 release-gate audit

Status: **RELEASED — HISTORICAL REQUIRED-CI VERIFICATION INCOMPLETE**

Date: 2026-08-22

This document records the historical v0.5.0 release gate and its final release
state. v0.5.0 was published and its immutable annotated tag peels to
`b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786`. Windows release validation passed.
The required Ubuntu CI did not complete successfully because of the lint-only
unused `std::fs::File` import described below. Recovery proceeded through
v0.5.1; the v0.5.0 tag and GitHub Release were not moved, deleted, recreated,
or otherwise changed.

## 1. Release boundary

Target version: `v0.5.0`

Codex baseline: exactly `codex-cli 0.149.0`

Verified platform: Windows only

The v0.5 capability is one host-authorized `repo.patch` call: conditionally
replace exactly one literal text occurrence in one bounded existing,
HEAD-tracked, unstaged, strict-UTF-8 worktree file. It is composed by a trusted
static profile and dispatched through the ordinary RAH `ToolRegistry` and
Generic Tool Bridge.

`RepositoryWorktreeMutationPolicy` is private and host-owned. A model request
and `PermissionLevel::Execute` are necessary runtime inputs but are not
worktree-write authority. The accepted state-plane boundary remains:

```text
worktree content mutation != index mutation != history/ref mutation
```

## 2. Accepted architecture decisions

The accepted ADR inventory is ADRs 0001 through 0012. ADR 0012 is Accepted and
defines the private worktree-content mutation authority. ADR 0010 remains
index-only; ADR 0011 remains a trusted-profile composition boundary only. No
provider-specific type crosses a RAH public boundary, and `rah-protocol`
remains dependency-bottom.

## 3. Evidence carried into the release gate

| Evidence | Status | Release meaning |
| --- | --- | --- |
| Deterministic patch foundation and hardening (Tasks 048–049) | Passed | Exact one-file literal replacement, complete-file SHA-256/byte-length preconditions, request/file/postimage bounds, strict UTF-8, BOM/CRLF preservation, repository/path/link/reparse/hard-link protections, same-parent temporary postimage, one-attempt semantics, and known-versus-uncertain outcome handling. |
| Trusted-profile composition (Task 050) | Passed | Static validation is non-spawning; the real effective composer resolves symbolic host resources and constructs `repo.patch` without widening its private policy. |
| Generic Tool Bridge verification (Task 051) | Passed | The real composer retains canonical `repo.patch`, private `rah_tool_0` aliasing, `Execute` permission, request denial before invocation, one execution, and no replay. |
| Windows live validation (Task 052) | Passed | Exact `codex-cli 0.149.0` used a fresh trusted-profile fixture, real effective composer, one tool request/lifecycle and one native replacement attempt. |
| Milestone audit (Task 053) | Passed | ADR 0012 Accepted; non-blocking limitations documented; no Windows-baseline release blockers. |

## 4. Live release gate

The fresh-fixture release live gate passed with:

```powershell
cargo run -p rah-runtime-codex --example live_trusted_profile_repo_patch_bridge
```

It used `codex-cli 0.149.0`, a trusted static profile, and the real effective
composer. It observed canonical `repo.patch`, private `rah_tool_0`, and
`PermissionLevel::Execute`; exactly one `ToolRequested`, `ToolStarted`, and
`ToolFinished`; one invocation and one native replacement attempt; terminal
`Completed`; and final marker `RAH_REPO_PATCH_LIVE_OK`. The target changed once;
index/HEAD/refs and the unrelated file were unchanged; no temporary sibling
remained; the app-server was reaped; the temporary repository was removed; and
Codex-owned filesystem, shell, process, MCP, network-tool, web, image, apps,
and approvals capabilities stayed disabled.

## 5. Security invariants and explicit deferrals

The policy has no generic write or process authority. It permits neither file
creation/deletion/rename/move, binary edits, multi-file transactions, staged or
untracked targets, restore-worktree, Git history/ref mutation, nor network Git.
It has no rollback guarantee and cannot completely eliminate TOCTOU races.
Uncertain effects are never automatically replayed. The release makes no Unix
live-validation claim.

The v0.3/v0.4 capability set remains available: Generic Tool Bridge, `fs.read`,
hardened local stdio MCP, hardened Process Plugin, trusted-profile built-ins,
trusted-profile MCP and Process Plugin composition, and mixed-provider lifecycle
cleanup. Codex-owned capabilities remain restricted; Codex is an optional
adapter, not an authority source.

## 6. Required release commands

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
cargo test -p rah-tools
cargo test -p rah-runtime-codex
cargo test -p rah-cli
cargo run -p rah-runtime-codex --example live_trusted_profile_repo_patch_bridge
```

`cargo metadata` must show every workspace package at `0.5.0`, no accidental
new crate edge, and only the already approved `sha2`, `uuid`, and `windows-sys`
usage. The deterministic suite requires no live model, credentials, network,
paid API, or GPU; the final command is opt-in and may use live model access.

## 7. Release checklist

| Check | Status |
| --- | --- |
| Workspace manifests and lockfile resolve to `0.5.0` without dependency upgrades | Passed |
| Exact local Codex executable reports `codex-cli 0.149.0` | Passed |
| Full deterministic workspace release commands | Passed |
| Focused `rah-tools`, `rah-runtime-codex`, and `rah-cli` suites | Passed |
| Fresh-fixture v0.5 live `repo.patch` gate | Passed |
| Only intended release-preparation files; no fixture/temp artifacts | Passed |
| Release-preparation commit created and clean-tree checks repeated | Passed |
| `v0.5.0` tag created at `b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786` | Passed |
| GitHub Release published | Passed |
| Required Ubuntu CI | Did not complete: clippy-only unused `File` import failure |
| Public tag and release preserved during recovery | Passed |

## 8. Historical release outcome

v0.5.0 is a published feature release, not a withdrawn or retagged release.
Its complete `repo.patch` milestone evidence and Windows live release gate
remain valid. The required Ubuntu CI failure was limited to the unconditional
`std::fs::File` import, which is used only by the Windows native-identity path;
it was not a repository-patch correctness, authority, or security failure.

The minimal `#[cfg(windows)]` import correction was made in
`6aae5b1fb710cb9d84fc1bcc51bddb9d1be9e22e`, whose Ubuntu CI passed. v0.5.1
then became the portability-only patch recovery release and the fully
required-CI verified v0.5.x baseline. This v0.5.0 record makes no Unix live
Codex validation claim.
