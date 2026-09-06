# RAH v0.18.0 Release Gate

**RELEASE PREPARATION — NOT TAGGED OR PUBLISHED**

## Release identity

- Release: `RAH v0.18.0`.
- Milestone: Inert Trusted Profile Persistence and Explicit Restore.
- Historical milestone audit: `95a460714115591b4aab2a287f9ba287a3336de1`;
  CI `34004857262` PASS; decision **READY FOR RELEASE VERIFICATION /
  PREPARATION**.
- Accepted release-preparation checkpoint:
  `c42fd36bfe31762a746eda2cf1c87838d5dfd7fe` (`fix: parse paginated cleanup
  tags`). It is a one-commit-later Actions Cleanup-only maintenance change, not
  v0.18 product capability evidence.
- Actions Cleanup run `34005428084`: completed/success on `c42fd36`.
- Tag: no `v0.18.0` tag exists.
- GitHub Release: no v0.18.0 GitHub Release exists.

## Product and authority contract

v0.18 persists one host-selected Trusted Profile source path as inert Desktop
preference. At restart it is remembered-not-restored: no profile selection,
validation, composition, provider spawn, Tool advertisement, or authority is
restored. Explicit Restore fresh-loads and statically validates the source
without spawning. Only explicit Connect/reconnect activates providers, and it
fresh-loads current source bytes before composition. Forget removes only the
durable preference and does not alter an already connected composition.

Profile generation participates in runtime currentness. The preference remains
host-owned desired state; it is not model-facing authority and does not alter
Trusted Profile composition, ToolRegistry dispatch, permissions, admission, or
lifecycle ownership. v0.18 adds no active-provider auto-restore, hot reload,
credential persistence, network MCP, provider installation/update, generic
shell/filesystem/Git authority, OS sandbox, network isolation, or rollback.

## Evidence chronology

| Item | Classification | Release interpretation |
| --- | --- | --- |
| Task 207 | INCONCLUSIVE / externally blocked | Model-selected external Tool execution was not established; the limitation remains unchanged. |
| Task 217 initial run | INCONCLUSIVE | A legacy v1 local Codex baseline store lacked the required v2 companion contract. |
| Task 217A | PASS supporting maintenance | Explicit host-only baseline repair restored the certified baseline v2; no product authority changed. |
| Task 217 resumed run | PASS | Windows Trusted Profile persistence lifecycle is live-certified within its stated scope. |
| Task 218 | READY | Milestone audit authorized release verification/preparation. |

The certified baseline remains exactly `codex-cli 0.149.0`, with certified
`codex.exe` SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
`codex-cli 0.153.4` remains research-only compatibility evidence. The baseline
v2 repair workflow is explicit and host-only, never automatic.

## Preserved limitations

This gate does not claim model-selected MCP or Process Plugin execution, live
external ToolStarted/ToolFinished handling, live external repository-effect
handling, Linux Desktop lifecycle certification, absence of ambient provider
effects, OS sandboxing, network isolation, rollback, or automatic profile
activation. Task 207 remains non-blocking only within that exact limitation.

## Workspace and dependency gate

- Exactly 13 workspace packages, each at `0.18.0`.
- Every workspace package retains Rust edition `2024`.
- No dependency additions, removals, or third-party version drift.
- `Cargo.lock` changes are limited to workspace package version references.
- No ADR, dependency direction, or production authority-semantics change is
  introduced by release preparation.

## Validation and publication checklist

- [x] `scripts/codex-baseline.ps1 verify 0.149.0`: PASS.
- [x] `cargo fmt --check`: PASS.
- [x] `cargo check --workspace`: PASS.
- [x] `cargo test --workspace`: completed with the pre-existing Windows
  foreign-owner diagnostic fixture debt recorded by Tasks 217/218; no product
  code was changed to mask it.
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- [x] `cargo metadata --no-deps --format-version 1`: 13 packages, all `0.18.0`,
  edition `2024`.
- [x] `git diff --check`: PASS.
- [ ] Preparation commit exact-head CI PASS recorded.
- [ ] `HEAD == origin/master` and clean worktree confirmed.
- [x] No v0.18.0 tag exists during preparation.
- [x] No v0.18.0 GitHub Release exists during preparation.
- [ ] Publication is separately authorized; Task 220 remains not started.
