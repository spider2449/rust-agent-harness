# RAH v0.35.0 release gate - final authority-first release

Status: **FINAL RELEASE OF THE AUTHORITY-FIRST ARCHITECTURE (LEGACY LINE)**.

v0.35.0 closes RAH's original "host-owned authority, fixed extension points"
architecture. After this release, `master` pivots to a plugin kernel
("everything is a plugin"); see the
[plugin-kernel pivot plan](../plans/2026-10-11-plugin-kernel-pivot-plan.md).
The legacy line is preserved at annotated tag `v0.35.0` and branch
`legacy/authority-first`. That branch accepts reference use and critical fixes
only; no feature work continues there.

## Scope and identity

- Base: `8c4e69777da8f13d2b86ab788da0934f2011e8b6` (local HEAD equal to
  `origin/master`; exact-head CI run 38135218466 passed before preparation).
- Content: the 17 commits after `v0.34.0` (see CHANGELOG), plus the version
  alignment and the release documentation in this commit.
- All 14 workspace packages, internal `Cargo.lock` records and the Desktop Tauri
  bundle version aligned to 0.35.0. No external dependency or checksum drift.
- No dependency edge, accepted ADR, authority category, Tauri command surface
  or HostExplicit route changed. HostExplicit remains exactly 11.

## Local validation (Windows, executed during preparation)

| Command | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS (3 pre-existing `rah-desktop` warnings under default features) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace --no-fail-fast` | **PARTIAL**: 1079 passed, 44 failed, 18 ignored. All failures are in the `rah-desktop` bin tests (326 passed / 44 failed); every other crate passes. |
| `git diff --check` | PASS |

### Known local `rah-desktop` test failures (pre-existing, not release regressions)

- **43 tests fail with `RepositoryMemberStale`** during repository activation
  (families: `task349_*` 12, `task351_*` 3, activation/remembered/prepared/target
  races and others). The same failure reproduces deterministically on an
  unmodified checkout of `8c4e697` (pre-release HEAD) **and at `v0.34.0`**, so
  it is not introduced by the 17 post-v0.34 commits or by this release
  preparation. It is environment-dependent on this Windows host; root cause was
  not investigated because Desktop is scheduled for removal in the pivot
  (plan phase P5).
- **1 test, `repository_snapshot_matrix_isolated_repositories_and_replacements`,
  failed once with `StagedDiffExecution`** and passed in the previous full run;
  treated as flaky.
- CI runs on `ubuntu-latest`, where the Windows-only `rah-desktop` crate is not
  exercised, so CI does not cover these tests.

**DESKTOP_WINDOWS_TESTS_NOT_GREEN** is recorded as a release limitation. The
human requested publication of this final legacy snapshot; because the failures
pre-exist in the published v0.34.0 as well, they are documented rather than
blocking. This record is not a claim that Desktop tests pass.

## Known non-certifications (unchanged from v0.34.0)

- **OPENAI_LIVE_NOT_VERIFIED**: real OpenAI Responses API acceptance is not
  certified; deterministic fixtures are not live PASS.
- **NSIS_INSTALLER_NOT_VERIFIED**: no installer or executable assets are
  published. Source-only release (GitHub source archives).
- Cross-platform live behavior is not certified.
