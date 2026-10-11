# RAH v0.17.0 Release Gate

**RELEASED — HISTORICAL RELEASE RECORD**

## 1. Release identity

- Release: `RAH v0.17.0`.
- Status: `RELEASED`.
- Milestone: Desktop Host-Selected Trusted Profile External Provider
  Integration.
- Immutable release commit: `dc9ae03598f1ac48a571bb118ae9fd971250a2b7`.
- Task 209 exact-head CI: `33958718038` PASS.
- Annotated tag: `v0.17.0`.
- Tag object: `8bf0bbeb14f3d7e42f1f53f2ee5c6098d561a4aa`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.17.0>.
- Release ID: `383204639`.
- Draft: `false`.
- Prerelease: `false`.
- Tag-triggered CI: `33959105671` PASS.

The v0.17.0 tag and GitHub Release are immutable publication evidence. Later
documentation-only commits, including Task 211, must not move or recreate the
tag, retarget the release, or become the tag target.

The release covers the completed v0.17 scope from Tasks 201-208: a
provider-only Desktop Trusted Profile overlay, inert static selection, explicit
Connect/reconnect activation, shared composition, local stdio MCP, Process
Plugin admission, host-owned permissions, fresh first-party/external registry
composition, sanitized Effective Authority descriptors, owned lifecycle
cleanup, and conservative external-effect handling.

## 2. Immutable prior v0.16.0 release

- Release commit: `509a5ba8daefeabbf91da50853402a1661099668`.
- Annotated tag: `v0.16.0`.
- Tag object: `c6ada41ed3c5edc677e392597c7d65dd5e9e69de`.
- GitHub Release: published for the immutable `v0.16.0` tag.

These identities remain unchanged.

## 3. Task chronology

| Task | Result |
| --- | --- |
| 201 | COMPLETE |
| 202 | COMPLETE |
| 203 | COMPLETE |
| 204 | COMPLETE |
| 205 | COMPLETE |
| 206 | COMPLETE |
| 207 | **INCONCLUSIVE / EXTERNALLY BLOCKED FOR EXECUTION SUB-GATE** |
| 207A | **INCONCLUSIVE** |
| 207B | **PASS** |
| 207C | **PASS** |
| 208 | **COMPLETE / milestone READY** |
| 209 | **COMPLETE / release prepared** |
| 210 | **COMPLETE / release published** |

Task 208 exact-head starting CI was `33956624643` PASS. Its pre-existing
Windows foreign-owner fixture issue remains recorded as technical debt and is
not silently converted into a v0.17 product failure or hidden from validation.

## 4. Product and authority contract

The host selects a provider-only Trusted Profile overlay. Static profile
selection and validation are inert/non-spawning. Providers activate only during
explicit Connect/reconnect, through the shared `rah-profile-composition` path.
The composer performs exact Tool-set/schema admission, preserves host-selected
permissions, merges admitted external Tools with the first-party Desktop Tool
registry, and fails closed on duplicate public Tool names. Provider lifecycle
ownership lasts for the usable registry lifetime and cleanup is host-owned.

Effective Authority presents sanitized external descriptors. Configured,
Effective, Advertised, and Current state remain separate. Tool advertisement,
provider metadata, frontend state, `PermissionLevel`, or a model request is not
authorization. `repositoryBound=false` and `PermissionLevel` do not prove an
external provider has no ambient repository or host effects.

ToolStarted revokes reviewed-commit authorization, ToolFinished refreshes the
selected repository, and started-without-finish is treated conservatively as
uncertain. Uncertain external effects are not automatically replayed or
claimed rolled back. Process supervision is not OS sandboxing.

## 5. Task 207 execution limitation

Task 207 remained **INCONCLUSIVE / externally blocked for the model-selected
external Tool execution sub-gate**. Windows provider selection, admission,
composition, effective inventory, dynamic Tool advertisement, lifecycle
ownership, and cleanup were verified.

Not established:

- model-selected MCP Tool execution;
- model-selected Process Plugin Tool execution;
- live external ToolStarted/ToolFinished lifecycle;
- live external-effect review invalidation;
- live external-effect repository refresh;
- full Windows model-driven external execution;
- Linux external-provider live certification.

The hardened certification failed closed at `ToolRequested=0`. No RAH product
defect was found, and Codex baseline migration was not justified. The
deterministically verified external-effect review invalidation and repository
refresh do not become live certification through a real external provider model
call. This limitation was accepted as non-blocking by Tasks 207C and 208.

This historical record does not claim Task 207 PASS or live-certified external
MCP, Process Plugin, Windows model-driven, or Linux external-provider
execution.

## 6. Platform and live evidence

- Windows provider selection, admission, composition, effective inventory,
  advertisement, lifecycle ownership, and cleanup: verified.
- Model-selected external MCP/Process Plugin execution: not established.
- Linux external-provider live certification: not established.
- Certified Codex baseline: `codex-cli 0.149.0`.
- Certified `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Current live-gate model: `gpt-5.6-terra`.
- `codex-cli 0.153.4`: research-only compatibility evidence, not certified;
  it did not restore Tool selection.

## 7. Workspace and dependency gate

- Exactly 13 workspace packages.
- Every workspace package version: `0.17.0`.
- Every workspace package uses Rust edition `2024`.
- No dependency additions or removals from the Task 208 starting point.
- No third-party source or version drift.
- `Cargo.lock` changes are limited to the 13 RAH workspace package version
  references changing from `0.16.0` to `0.17.0`.
- The Task 207A certification-hardening dependency state remains valid,
  including its established `uuid` lock/dependency entry.
- No new ADR or production authority semantics are introduced.

## 8. Validation and known fixture issue

Task 209 recorded the required release-preparation validation:

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test --workspace`: accepted with the known Windows fixture exception.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `git diff --check`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS; exactly 13 workspace
  packages, all `0.17.0`, edition `2024`.
- Focused provider, composition, runtime, frontend, and Desktop checks were
  recorded in the Task 209 preparation evidence.

The known failure is exactly
`tests::hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic`.
The assertion was `the diagnostic must reproduce Git's protected ownership
refusal` at `crates/rah-desktop/src/main.rs:6438`; the focused rerun reproduced
the same result. It is the pre-existing Windows/Git fixture portability issue
documented by Task 208, not a v0.17 product regression. No product code was
changed to mask it.

Task 211 is documentation-only post-release cleanup. No new live external
provider Tool run is required and no live evidence is manufactured.

## 9. Completed release record

- [x] v0.17.0 release candidate prepared.
- [x] Task 209 exact-head CI passed: `33958718038`.
- [x] Immutable `v0.17.0` annotated tag created and verified.
- [x] GitHub Release published and verified: release ID `383204639`.
- [x] Tag-triggered CI passed: `33959105671`.
- [x] Task 207 limitation preserved without an external execution claim.
- [x] Task 211 post-release documentation cleanup recorded separately.

The v0.17.0 release remains permanently attached to
`dc9ae03598f1ac48a571bb118ae9fd971250a2b7`. Task 211 is a later docs-only
master commit and must never become the v0.17.0 tag target.

The next task is a new milestone scope/roadmap task. Task 212 is not started
automatically.
