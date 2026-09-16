# RAH v0.27 Live Certification

Date: 2026-09-16

## Verdict

`PASS WITH EXPLICIT NONCLAIMS`

The v0.27 remembered-workspace security, privacy, persistence, fresh
admission, restart, and explicit activation gate passed on Windows through the
production Desktop backend/state path.

Certified production source:

`b90e63dd138732d616582d935b1fa45b842fcfe8`

Exact-head CI run `35054734280` completed successfully for that full SHA.
At certification time `HEAD == origin/master` and the worktree was clean.

The gate was host-driven, not GUI-driven. Real junction/reparse and true
two-process simultaneous live subcases were not executed. Deterministic
production tests passed for those safety contracts, and no live claim is made
for the omitted optional subcases.

## Evidence summary

- P1 remembered A/B, revealed and hid location explicitly, reordered, admitted
  without activation, then explicitly activated A.
- P2 restarted from the same app-data root with A/B descriptive only: zero
  membership, no active repository, no composition, no commit/stage state, no
  HostExplicit state, no provider/runtime connection, and no executable
  conversation binding.
- P2 re-admitted A with a fresh `RepositoryMemberId`, kept A active while
  admitting B, and switched only through explicit activation.
- Deleting remembered B while B was active removed durable catalog B only;
  process-local membership, active repository, composition, and repository
  files remained.
- P3 confirmed deleted B did not return and authority was not restored.
- Invalidated C remained descriptive but was rejected by current admission
  validation.
- Corrupt and future-version catalogs remained unchanged and unavailable,
  with no partial candidates or authority.
- Candidate startup path access count was zero.
- Privacy sentinel scan passed and Git integrity for A/B passed.
- Model requests, model Tool requests, MCP, Process Plugin, automatic commit,
  and network Git counts were all zero.
- Required marker emitted:
  `RAH_V027_REMEMBERED_WORKSPACE_LIVE_OK`

## Source and environment

```text
CERT_SOURCE_SHA=b90e63dd138732d616582d935b1fa45b842fcfe8
origin/master=b90e63dd138732d616582d935b1fa45b842fcfe8
worktree=clean
exact-head CI=PASS, run 35054734280
Windows=Windows 10 Professional build 19045, x64
Rust=rustc 1.96.0 (ac68faa20 2026-05-25)
Cargo=cargo 1.96.0 (30a34c682 2026-05-25)
Git=git version 2.54.0.windows.1
Workspace=13 packages, version 0.26.0, edition 2024
HostExplicit=11
Codex certified baseline=0.149.0
Codex baseline SHA256=14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00
Ambient Codex=0.154.0, unused
```

## Validation

Passed:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test --workspace -- --test-threads=1`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`
- frontend syntax, remembered-workspace, and Tauri permission tests
- `cargo build -p rah-desktop --release`
- exact-head CI run `35054734280`

The detailed invariant matrix, audit findings, deterministic evidence, live
steps, cleanup evidence, and explicit nonclaims are recorded in
`docs/plans/2026-09-16-task-335-v0.27-security-audit-live-certification.md`.

## Permanent nonclaim

`MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED GPT-5.6-TERRA LIVE GATE.`

Next task after evidence acceptance: **Task 336 — RAH v0.27 Milestone Audit
and Release Preparation**.
