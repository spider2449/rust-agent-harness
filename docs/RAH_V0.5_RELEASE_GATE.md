# RAH v0.5 release-gate audit

Status: **RELEASE CANDIDATE PREPARED — NOT PUBLISHED OR TAGGED**

Date: 2026-08-22

This document records the local v0.5 release gate. It prepares a release only;
it does not create `v0.5.0`, push, publish a GitHub Release, or validate CI.

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
| `v0.5.0` tag created | Not performed |
| CI/GitHub Release published | Not performed |

When the pending deterministic and live gates pass, the next authorized task is
Task 055: create and verify the `v0.5.0` tag and publication state, then stop
before post-release cleanup.
