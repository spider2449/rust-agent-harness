# RAH v0.17.0 Release Gate

**RELEASE CANDIDATE PREPARED - NOT YET RELEASED**

## 1. Release identity

- Release: `RAH v0.17.0`.
- Status: prepared, not yet released.
- Milestone: Desktop Host-Selected Trusted Profile External Provider
  Integration.
- Release-preparation commit: the Task 209 preparation commit; its exact SHA
  is recorded in the Task 209 completion report.
- Exact-head CI: required for the release-preparation commit; no earlier CI
  run substitutes for it.
- No `v0.17.0` tag or GitHub Release may be created by Task 209.

The candidate covers the completed v0.17 scope from Tasks 201-208: a
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

These identities must remain unchanged. Task 209 does not move or recreate
the prior release.

## 3. Required v0.17 evidence chain

| Task | Evidence / status |
| --- | --- |
| 201 | v0.17 scope and authority roadmap; provider-only Desktop integration selected with existing authority boundaries preserved. |
| 202 | Trusted Profile composition contract; shared composition, lifecycle, and conservative external-effect rules defined; no new ADR. |
| 203 | `rah-profile-composition` extracted and consumed by CLI/Desktop without inverting provider/runtime boundaries. |
| 204 | Desktop provider-only selection and inert static validation implemented; selection does not spawn or require an executable. |
| 205 | Connect/reconnect activation, mixed MCP/Process Plugin composition, fresh registry publication, permission preservation, and provider ownership implemented. |
| 206 | External Effective Authority descriptors and conservative external-effect lifecycle handling hardened. |
| 207 | **INCONCLUSIVE / externally blocked for the model-selected external Tool execution sub-gate.** Provider selection, admission, composition, inventory, advertisement, lifecycle ownership, and cleanup were verified; model-selected execution was not established. |
| 207A | **INCONCLUSIVE.** Hardened certification correctly failed closed at `ToolRequested=0`; no model-selected external execution was established. |
| 207B | **PASS** for compatibility research. Codex 0.149.0 and research-only 0.153.4 comparison both produced `0/0/0`; no migration basis was found. |
| 207C | **PASS** for disposition. Option B permits release consideration with the explicit live limitation; it is not a PASS for Task 207. |
| 208 | **COMPLETE / READY FOR RELEASE VERIFICATION/PREPARATION.** No release-blocking RAH defect was found. |

Task 208 exact-head starting CI: `33956624643` PASS. Its pre-existing
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

Actual model-selected MCP/Process Plugin Tool execution was not established
with the tested current ChatGPT-auth Codex model/runtime combinations. The
hardened certification gates failed closed at `ToolRequested=0`. No RAH
product defect was found, and Codex baseline migration was not justified.

Therefore real external-effect review invalidation and repository refresh are
deterministically verified but not live-certified through a real external
provider model call. This is the known non-blocking limitation accepted by
Task 207C and Task 208. This gate does not claim Task 207 PASS or external MCP,
Process Plugin, or Windows model-driven execution certification.

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

## 8. Required validation

Pre-commit validation must run:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`
- `cargo test -p rah-profile-composition`
- `cargo test -p rah-tools-mcp`
- `cargo test -p rah-tools-plugin`
- `cargo test -p rah-runtime-codex`
- `cargo test -p rah-desktop`
- `node --check crates/rah-desktop/frontend/status.js`
- `node --check crates/rah-desktop/frontend/status_authority_test.js`
- `node crates/rah-desktop/frontend/status_authority_test.js`
- `cargo build -p rah-desktop --release`

### Task 209 pre-commit results

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test --workspace`: ACCEPTED WITH THE KNOWN PRE-EXISTING WINDOWS
  FIXTURE EXCEPTION below; `rah-desktop` reported 156 passed, 1 failed, and
  2 ignored, and the workspace stopped at that package.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `git diff --check`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS; exactly 13 workspace
  packages, all `0.17.0`, edition `2024`.
- `cargo test -p rah-profile-composition`: PASS; 4 tests.
- `cargo test -p rah-tools-mcp`: PASS; 31 integration tests plus 1 unit test.
- `cargo test -p rah-tools-plugin`: PASS; 15 integration tests plus 6 unit
  tests.
- `cargo test -p rah-runtime-codex`: PASS; 82 unit/bridge tests, 6
  architecture tests, and 11 live-gate contract tests; 1 ignored.
- `cargo test -p rah-desktop`: ACCEPTED WITH THE SAME KNOWN FIXTURE
  EXCEPTION; 156 passed, 1 failed, and 2 ignored.
- `node --check crates/rah-desktop/frontend/status.js`: PASS.
- `node --check crates/rah-desktop/frontend/status_authority_test.js`: PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js`: PASS.
- `cargo build -p rah-desktop --release`: PASS.

The full workspace and focused Desktop failure is exactly
`tests::hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic`.
The assertion was `the diagnostic must reproduce Git's protected ownership
refusal` at `crates/rah-desktop/src/main.rs:6438`. The exact focused rerun
reproduced the same failure. This is the pre-existing Windows/Git fixture
portability issue documented by Task 208, not a v0.17 product regression; no
product code was changed to mask it. Task 208's authoritative exact-head CI
was PASS, and Task 209 requires its own exact-head CI after the preparation
commit.

The known Windows test is
`tests::hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic`.
If it reproduces, record the exact failure and focused rerun, classify it as
the pre-existing Task 208 fixture issue unless new evidence shows a regression,
and do not change product code merely to make release preparation green.

## 9. Release checklist

- [x] Workspace version bump and lockfile audit prepared.
- [x] Release-facing documentation states prepared/not yet released.
- [x] Task 207 limitation is preserved without an external execution claim.
- [x] Full pre-commit validation recorded, with the Task 208-classified
      Windows fixture exception documented above.
- [ ] Intended Task 209 files committed as `docs: prepare RAH v0.17.0 release`.
- [ ] `master` pushed and `HEAD == origin/master` verified.
- [ ] Exact-head CI for the preparation commit passes.
- [ ] Post-commit metadata, dependency, documentation, tag, release, and clean
      worktree checks pass.
- [ ] No `v0.17.0` tag created.
- [ ] No GitHub Release published.

Only after this gate is complete may Task 210 - v0.17.0 Release Publication
begin. Task 210 is not started automatically.
