# RAH v0.12 Release Gate

**RELEASE PREPARATION — NOT YET RELEASED**

## Candidate identity

- Candidate release: `RAH v0.12.0`.
- Release-preparation starting checkpoint:
  `62d4adba6fe2c7cb1893b405b6964c38bb95352c`
  (`docs: audit v0.12 Desktop authoring milestone`).
- Task 152 exact-head CI: `33477249271` — PASS.
- Eventual release commit / peeled tag target: PENDING until the Task 153
  release-preparation commit exists.
- Annotated tag `v0.12.0`: NOT CREATED.
- Tag object: NOT CREATED.
- Task 153 exact-head CI: PENDING in this committed candidate document.
- GitHub Release: NOT PUBLISHED.
- Certified Codex baseline: exactly `codex-cli 0.149.0`.
- Verified live platform: Windows.

RAH v0.11.0 remains the last released version while this candidate is prepared.

## Authority summary

```text
model bounded repository authoring
  -> human Stage / Unstage host actions
  -> host-observed staged review
  -> human reviewed-snapshot authorization
  -> message-only repo.commit request
  -> ADR 0016 revalidation and one fixed native Git attempt
  -> verified result and Desktop refresh
```

v0.12 productizes existing boundaries through Desktop and introduces no new
authority. Model request is not authorization. Execute permission is not commit
authorization. The frontend does not own authorization. Human Stage / Unstage
remain host actions, and human Authorize is the reviewed-snapshot authorization
event. `RepositoryCommitReview` remains opaque and Rust-only. `repo.commit`
remains message-only and does not auto-stage. Uncertain external effects are
not replayed.

## Task 144–152 evidence

| Task | Commit | CI / evidence | Result |
| --- | --- | --- | --- |
| 144 — scope roadmap | `aa2f2d32d7c83033cbd7ff4d24abd64c96f3330c` | `33302269295` PASS | Desktop workflow selected; no new authority required. |
| 145 — integration research | `27e9247bd3653240c679f95d775a2884c47ec11b` | Recorded gates PASS | Direct `rah-tools` composition safe; no `rah-cli` dependency. |
| 146 — shared composition | — | Not applicable | **SKIPPED BY DESIGN**; direct composition was feasible. |
| 147 / 147A — review and hardening | `669a4b36751121e137a539398e21d603a4c7ca98` / `cb836446593ec8f3f79178196bce90fd18c4993a` | `33304029128` PASS / recorded gates PASS | Host actions, complete textual review, binary refusal, bound selectors and digest. |
| 148 — reviewed authorization | `fd16d8fc42867b1777514be5a56da1ca2b9748d6` | Recorded gates PASS | Opaque Rust-only compare-and-arm control. |
| 149 — Desktop commit integration | `9beb057377fbe529c91a260208eb83504b6d27c7` | Recorded gates PASS | Exact paired Tool/control, sanitized verified result, refresh. |
| 150 — deterministic hardening | `499f726c0be6b977a9ca716e12e49cb62bb65a9c` | Recorded gates PASS | Execute-not-authorization and cross-layer one-shot proof. |
| 151 — Windows live validation | `1021b1fb1f2662f6c59002a14119ab3cde37cba3` | `33463666613` PASS | Live workflow and certified-bundle v2 repair. |
| 152 — milestone audit | `62d4adba6fe2c7cb1893b405b6964c38bb95352c` | `33477249271` PASS | **READY FOR v0.12 RELEASE PREPARATION**. |

## Task 151 Windows live evidence

- Minimal runtime smoke: `RAH_ECHO_BRIDGE_OK`.
- Byte-clean live repository: `D:\rah-task151-clean`.
- Live fixture commit: `90683f5eaab129a75e815879e69586ff75de5e86` with
  message `RAH Task 151 live commit`. It is fixture evidence, not the RAH
  v0.12 release commit.
- Displayed/model-reported OID equalled actual fixture HEAD; parent equalled the
  exact old HEAD; commit count changed from 1 to 2; `HEAD:tracked.txt` was
  exactly `RAH_TASK151_EDIT_OK` with an LF terminator; the branch advanced to
  the exact new HEAD; and worktree/index were clean.
- Exactly one Git-observable commit effect was independently verified. No
  second commit or replay was observed.

The bounded repository-safe authoring path was live-proven. The exact live edit
Tool label was not durably retained, and exact live `repo.commit`
`ToolRequested` / `ToolStarted` / `ToolFinished` counts were not durably
retained. These are documented non-blocking observability gaps; this gate does
not invent a Tool label or `1 / 1 / 1` activity-event claim.

## Certified Codex bundle v2 hardening

The certified baseline remains exactly `codex-cli 0.149.0`. Closed manifest
schema v2 requires the demonstrated complete sibling pair:

- `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.

The v2 verifier validates separate identities/hashes, PE identity, canonical
containment, and no reparse points, failing closed for invalid existing
certified directories. Task 151 closed the Desktop verifier gap that could
accept an incomplete certified directory containing `codex.exe` but missing
`codex-code-mode-host.exe`; it did not newly discover the general same-version
code-mode-host requirement already present in v0.11 evidence. No
`codex-windows-sandbox-setup.exe` or `codex-command-runner.exe` requirement is
claimed for this validated RAH path.

## Security invariants and deferred scope

- No generic Git, shell/process, or `fs.write` authority; no generic branch,
  ref, network, or credential Git authority.
- No automatic staging, no rollback guarantee, and no replay of uncertain
  external effects.
- ADR 0010 repository/index mutation boundary and ADR 0016 exact reviewed
  staged-snapshot commit authority remain authoritative; ADRs 0011–0015 also
  remain authoritative.
- Windows is live-certified. Unix/macOS live validation is not claimed.
- Task 120 remains **DEFERRED / NOT VALIDATED**:
  `RAH_TASK120_NETWORK_OK = NOT VALIDATED / DEFERRED`.
- Transport confinement remains **NOT CLAIMED**.

## Dependency record

v0.12 introduces no Cargo dependency. Task 153 must have
**DEPENDENCY DELTA = NONE**; only workspace-local `Cargo.lock` package-version
records required by the version bump are allowed.

## Deterministic release checklist

- [x] `cargo fmt --check`
- [x] `cargo check --workspace -j 1`
- [x] `cargo test --workspace -j 1`
- [x] `cargo clippy --workspace --all-targets --all-features -j 1 -- -D warnings`
- [x] `git diff --check`
- [x] `cargo metadata --no-deps --format-version 1` confirms 12 packages at `0.12.0`, edition 2024
- [x] `node --check crates/rah-desktop/frontend/status.js`
- [x] `cargo build -p rah-desktop --release`
- [x] `scripts/test-codex-baseline.ps1 -NativeCodex <certified-codex.exe>`
- [x] `scripts/codex-baseline.ps1 verify 0.149.0`
- [ ] Task 153 release-preparation commit created and pushed
- [ ] Local `HEAD == origin/master`
- [ ] Task 153 exact-head CI PASS

## Desktop release artifact record

- `rah-desktop.exe` path:
  `D:\spider\working\rust-agent-harness\target\release\rah-desktop.exe`.
- File size: `17,440,768` bytes.
- Last-write timestamp: `2026-09-01 14:44:18 +08:00`.
- SHA-256:
  `36b9594cd91e6a28d88b8334e05bcceeb303387f7d525e82d23658abac15acd6`.
- Adjacent `sqlite3.dll`: NO; SQLite remains bundled.

This document intentionally retains pending release-commit and exact-head-CI
fields in the candidate commit. Task 154/post-release work may convert the
actual immutable identities into historical release data after a tag and
publication exist.
