# RAH v0.5.1 release-gate recovery audit

Status: **RELEASED — FULLY REQUIRED-CI VERIFIED**

Date: 2026-08-22

This historical record confirms the completed v0.5.1 patch release. It did not
move, delete, recreate, or force-update the published `v0.5.0` tag or GitHub
Release.

## 1. Recovery boundary

Target version: `v0.5.1`

Codex baseline: exactly `codex-cli 0.149.0`

Release-preparation and peeled tag target:
`0ea648d84d6f48720c33e8b1bb07e1c24101c870`

GitHub Release: published (not draft, not prerelease)

The only source change is recovery commit
`6aae5b1fb710cb9d84fc1bcc51bddb9d1be9e22e`: `std::fs::File` is imported under
`#[cfg(windows)]` because its only production use is `File::from_raw_handle` in
the Windows-only native identity implementation. The unconditional import was
unused in the Ubuntu build and caused clippy to fail.

This is a portability-only import correction. It changes no `repo.patch`
authority, precondition, path restriction, replacement behavior, test
behavior, public API, crate dependency, profile, Codex adapter behavior, or
ADR.

## 2. Historical v0.5.0 state

`v0.5.0` is an immutable published release. Its annotated tag continues to peel
to release-preparation commit
`b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786`, and the GitHub Release remains
published. Windows validation passed for that release, but its required Ubuntu
CI run failed only at clippy because of the unused `File` import.

The v0.5 feature-milestone evidence remains valid: the narrow private
host-owned `RepositoryWorktreeMutationPolicy`, trusted-profile composition,
Generic Tool Bridge, and Windows live `repo.patch` gate are unchanged. This
recovery does not reinterpret that lint defect as a `repo.patch` correctness or
security failure.

## 3. Recovery CI evidence

The normal branch recovery commit was pushed without pushing or modifying any
release tag. GitHub Actions CI run `32573592369` for
`6aae5b1fb710cb9d84fc1bcc51bddb9d1be9e22e` completed successfully.

| Ubuntu step | Result |
| --- | --- |
| Check formatting | Passed |
| Check workspace | Passed |
| Test workspace | Passed |
| Lint workspace | Passed |

The workflow has one Ubuntu `deterministic-validation` job and no GitHub
Windows job. The prior `unused import: File` failure is absent. Windows remains
the separately verified live release platform.

## 4. v0.5.1 local release validation

The following commands passed at workspace version `0.5.1` before the
release-preparation commit:

```powershell
codex --version
# codex-cli 0.149.0

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

Cargo metadata resolves all 11 workspace packages to `0.5.1`. The lockfile
changes only those internal package version entries; it makes no dependency
upgrade or crate-edge change.

The rerun Windows live gate used exactly `codex-cli 0.149.0` and observed
canonical `repo.patch`, private alias `rah_tool_0`,
`PermissionLevel::Execute`, one ToolRequested/ToolStarted/ToolFinished event,
one invocation, one native replacement attempt, terminal Completed, and the
final marker `RAH_REPO_PATCH_LIVE_OK`. Index/HEAD/refs and unrelated content
were unchanged; the app-server was reaped; the fixture was removed; and
restricted Codex-owned capabilities remained disabled. No Unix live Codex run
is claimed.

## 5. Required release CI

The release-preparation commit passed its required GitHub CI run before
publication:

| CI evidence | Result |
| --- | --- |
| Release-preparation CI run `32574019502` | `completed / success` |
| `v0.5.1` tag CI run `32574129999` | `completed / success` |

These are the required Ubuntu deterministic-validation results. They do not
make a Unix live Codex validation claim.

## 6. Release checklist

| Check | Status |
| --- | --- |
| v0.5.0 tag and GitHub Release retained unchanged | Passed |
| Minimal recovery commit pushed without a tag update | Passed |
| Required Ubuntu recovery CI | Passed |
| Workspace version and internal lock entries at `0.5.1` | Passed |
| Local deterministic, focused, and live release gates | Passed |
| v0.5.1 release-preparation commit created | Passed: `0ea648d84d6f48720c33e8b1bb07e1c24101c870` |
| Required CI for that exact release-preparation commit | Passed: run `32574019502` |
| Annotated `v0.5.1` tag pushed at that commit | Passed |
| GitHub Release `RAH v0.5.1` published | Passed: not draft, not prerelease |
| Required tag CI | Passed: run `32574129999` |

v0.5.1 is published as a portability correction for v0.5.0's Ubuntu clippy
failure. It makes no feature or authority change relative to v0.5.0 beyond the
`#[cfg(windows)]` import correction, does not change `repo.patch` behavior, and
does not claim Unix live Codex validation. It is the fully required-CI verified
v0.5.x baseline.
