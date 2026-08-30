# RAH v0.11 Release Gate

Release status: **RELEASE PREPARATION — NOT YET RELEASED**

## Candidate identity

- Candidate version: `0.11.0`.
- Audited starting checkpoint: `15420bcffa0a2bb2155f7f691a9af17f07ca0894`.
- Task 140 exact-head CI: `33295380139` — PASS.
- Release-preparation commit: pending until Task 141 commit is created.
- Certified Codex baseline: exactly `codex-cli 0.149.0`.
- Verified live platform: Windows.
- Tag: **NOT YET CREATED**.
- GitHub Release: **NOT YET PUBLISHED**.

## Authority summary

```text
trusted host
  -> trusted static profile
  -> exact repository / native Git / host identity
  -> repo.commit capability
  -> explicit host-reviewed staged-snapshot authorization
  -> one fixed normal commit attempt
  -> verified result
```

Trusted Profile enablement alone does **not** authorize a commit. Execute alone
does **not** authorize a commit. The model controls only the commit message.

## Security invariants

- Public Tool: `repo.commit`; input: `message` only; `additionalProperties` is
  false; result is bounded/redacted.
- `RepositoryCommitControl` is host-only; authorization is in-memory and
  one-shot, with no SQLite persistence or restart reconstruction.
- The exact repository, native Git executable, and trusted host identity are
  bound. Admission requires attached non-unborn HEAD on the current existing
  branch and the exact reviewed compound index snapshot.
- No automatic staging; fixed commit command; hooks neutralized; signing
  disabled; host identity explicit; shared RAH repository mutation lease.
- Exactly one mutating attempt; no retry/replay; uncertain is not rollback; no
  external-actor serialization claim.

## Task evidence

| Task | Commit | CI | Result |
| --- | --- | --- | --- |
| 132 — scope/authority roadmap | `f8c2e4da835f0167a3ad35440fa825d501ba1bde` | `33277286536` | PASS |
| 133 — commit authority research | `982537d5203dd807627bbe6717066dff5fb52452` | `33277927682` | PASS |
| 134 — ADR 0016 accepted | `6b052ec070d492e58ae0de3eb49777c75324afd5` | `33278400023` | PASS |
| 135 — private commit foundation | `8497a1d55395b2f6bbe5cc0d6c1319b7e84114fc` | `33279186751` | PASS |
| 136 / 136A — deterministic hardening / CI lint recovery | `a128a53214cf35538ae2f57622e7a7d7b7597fb9` / `243abd5d7a6f5d3a504e956d8c365919609cd430` | `33283001044` / `33284211437` | lint recovery PASS |
| 137 — Trusted Profile composition | `e02cd3b6ebef789531c47b856e841a9df8e8b05f` | `33284958371` | PASS |
| 138 — Generic Tool Bridge verification | `c1a77bdc1a2a20afc677c2292fe4ed5a69e7100f` | `33290249642` | PASS |
| 139 — Windows certified live validation | `ad7db6d1067a05cb26d37198074375d300eb3e51` | `33294858193` | PASS |
| 140 — milestone audit | `15420bcffa0a2bb2155f7f691a9af17f07ca0894` | `33295380139` | PASS |

## Task 139 Windows live evidence

- Certified Codex: `0.149.0`; official `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Official same-version `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.
- Harmless dynamic-tool control: PASS, lifecycle `1 / 1 / 1`.
- `repo.commit` live path: PASS, lifecycle `1 / 1 / 1`; Tool result:
  `committed_verified`.
- Disposable fixture OID: `13c200c5c772b3e4a0eceb0a2364981c849313e0`.
  Model OID, ToolOutput OID, and actual fixture HEAD were equal. The fixture
  commit is **not** a RAH repository commit.
- No automatic staging, second commit, retry, replay, approval, or synthetic
  call; cleanup PASS. Markers: `RAH_REPOSITORY_COMMIT_LIVE_OK` and
  `LIVE_REPOSITORY_COMMIT_BRIDGE_PASS`.

## Codex packaging and platform status

The Windows certified dynamic-tool path used complete official `0.149.0`
runtime pieces, including same-version `codex-code-mode-host.exe`. A bare
standalone `codex.exe` is not claimed sufficient for all live dynamic-tool
operation. Installed Codex `0.150.1` was untouched and was not certified.

- Windows: live certified.
- Ubuntu: deterministic CI evidence.
- Linux live: not claimed.
- macOS live: not claimed.

## Deferred scope

Deferred: branch creation/switching; arbitrary refs; detached/unborn commits;
amend; merge/rebase/cherry-pick; reset/clean/stash; tags; push/pull/fetch;
credentials; network Git; linked worktrees; submodule/gitlink commit; generic
delete/rename; generic `fs.write`; generic shell/process; network
MCP/Streamable HTTP; PluginManager install/update; profile hot reload; dynamic
authority restoration; multi-repository authority; OS sandbox; network
isolation; and rollback.

`RAH_TASK120_NETWORK_OK = NOT VALIDATED / DEFERRED`. Transport confinement:
**NOT CLAIMED**.

## Dependency record

v0.11 introduced no new Cargo dependency. Task 141 verifies `Cargo.toml` and
`Cargo.lock` after the version bump; dependency delta must remain **NONE**.

## Deterministic checklist

- [x] `cargo fmt --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace -j 1`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] `git diff --check`
- [x] `cargo metadata --no-deps --format-version 1` confirms 12 packages at `0.11.0`
- [x] `node --check crates/rah-desktop/frontend/status.js`
- [x] `cargo build -p rah-desktop --release`
- [x] Desktop release executable inspected: `F:\coding\otherPrj\rust-agent-harness\target\release\rah-desktop.exe`; 16,707,584 bytes; `2026-08-30 14:05:56 +08:00`; adjacent `sqlite3.dll` absent.
- [ ] Task 141 exact-head deterministic-validation CI after push
