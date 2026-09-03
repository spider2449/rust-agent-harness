# RAH v0.14 Release Gate

**RELEASE PREPARATION — NOT YET RELEASED**

## 1. Release candidate identity

- Target: `v0.14.0`.
- Starting HEAD: `ffe091530c5b29fbef579d1fdf8ada4986937d2b`.
- Task 176 exact-head CI: run `33726778388` — PASS.
- Prepared release commit: TBD until Task 177 commit exists.
- Tag: not created.
- GitHub Release: not published.

## 2. Workspace metadata

- Expected workspace packages: 12.
- Expected package version: all `0.14.0`.
- Expected Rust edition: `2024`.
- Dependency drift: none; only workspace-local Cargo.lock version records may
  change.

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

## 10. Later release checklist

- [ ] Complete Task 177 local validation and candidate review.
- [ ] Commit and push the prepared release candidate.
- [ ] Verify exact-head CI passes for the Task 177 commit.
- [ ] Create annotated tag `v0.14.0` at the exact prepared commit (Task 178).
- [ ] Push and verify the immutable tag target (Task 178).
- [ ] Publish and verify the GitHub Release (Task 178).

No tag or GitHub Release has been created by Task 177.
