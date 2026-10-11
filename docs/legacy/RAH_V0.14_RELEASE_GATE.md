# RAH v0.14 Release Record

**RELEASED — HISTORICAL RECORD**

## 1. Release candidate identity

- Release: `RAH v0.14.0`.
- Starting HEAD: `ffe091530c5b29fbef579d1fdf8ada4986937d2b`.
- Task 176 exact-head CI: run `33726778388` — PASS.
- Status: `RELEASED`.
- Immutable release commit:
  `52506521bdf838784dd45bb54df2d6bcff8bcd08`.
- Annotated tag: `v0.14.0`.
- Tag object:
  `9193423e96dd0cda2fd8f5ed5619ab2b58483acc`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.14.0>.

## 2. Workspace metadata

- Workspace: 12 packages, all `0.14.0`, Rust edition `2024`.
- Dependency drift: none.

## 3. Scope and authority

v0.14 is the bounded repository file rename/move milestone under accepted ADR
0018. The public `repo.rename-file` Tool authorizes exactly one clean,
HEAD-tracked regular file to move within the same repository, either within
one directory or across existing directories. It requires the exact four-field
request (`source_path`, `destination_path`,
`expected_source_file_sha256`, `expected_source_file_byte_length`) and an
absent destination with no replacement.

Rename is a separate host-owned authority. It does not imply repository read,
content replacement, creation, deletion, index mutation, reviewed
commit/history mutation, Execute, generic filesystem, shell/process, or Git
authority. Model requests, provider metadata, Trusted Profile composition, and
frontend state are not authorization.

## 4. ADR authority

- ADR 0018: bounded repository file rename/move authority.

## 5. Deterministic validation

Commands and results:

- `cargo fmt --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace` — PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `git diff --check` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS; exactly 12 packages,
  all `0.14.0`, edition `2024`.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `cargo build -p rah-desktop --release` — PASS.

## 6. Windows live certification

Task 175 completed Windows live `repo.rename-file` certification with PASS.

- Final Task 175G closure commit:
  `2cd837ca39a653a86b5943b794aa3a5832116697`.
- Exact-head CI: run `33725840696` — PASS.
- Certified Codex: `codex-cli 0.149.0`.
- `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.
- Evidence included advertisement, actual alias, exact request, lifecycle
  `1 / 1 / 1`, `is_error=false`, `renamed_verified`, `uncertain=false`,
  marker, parseable JSONL, exact bytes/hash, and unchanged index/HEAD/refs.

## 7. Milestone audit

- Task 176 commit: `ffe091530c5b29fbef579d1fdf8ada4986937d2b`.
- Exact-head CI: run `33726778388` — PASS.
- Audit result: PASS — ready for v0.14 release preparation.

## 8. Platform qualification

Windows is live-certified. Ubuntu/Linux has deterministic validation through
tests and CI only; Linux live certification is not established.

## 9. Known limitations and deferrals

- No directory or recursive move, overwrite/replace, Windows case-only rename,
  untracked-file rename, or dirty-file rename.
- No generic `fs.rename`, generic filesystem mutation, shell/process authority,
  generic Git authority, or network Git.
- No rollback or transaction guarantee; no cross-volume copy-delete fallback.
- Process supervision is not OS sandboxing; network isolation is not claimed.
- Linux live certification is not established.

## 10. Completed release checklist

- [x] Release candidate prepared.
- [x] Exact-head CI passed: Task 177, run `33727731967`.
- [x] Annotated tag created: `v0.14.0` (Task 178).
- [x] Tag pushed (Task 178).
- [x] Immutable tag target verified (Task 178).
- [x] GitHub Release published (Task 178).
- [x] Release publication verified (Task 178).

Task 177 prepared the release candidate. Task 178 created and pushed the
annotated tag and published the GitHub Release. Task 179 records the completed
post-release state; it does not move the tag or republish the release.
