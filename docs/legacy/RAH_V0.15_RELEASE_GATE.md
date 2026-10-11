# RAH v0.15.0 Release Gate

**RELEASED — HISTORICAL RECORD**

## 1. Release identity

- Release: `RAH v0.15.0`.
- Status: `RELEASED`.
- Starting HEAD / Task 187 audit commit:
  `5e6ebd8b84d61b5018388c06df908553c5d77f33`.
- Task 187 exact-head CI: run `33828160578` — PASS; ready for v0.15 release
  preparation.
- Immutable release commit:
  `6b66a357cacea4b1fcf21131cbc9e72fab90d59c`.
- Annotated tag: `v0.15.0`.
- Tag object: `6ca031e66972b5e04dcade6766d6156a9c3e1a9b`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.15.0>.

## 2. Workspace metadata

- Expected: 12 workspace packages, all `0.15.0`, Rust edition `2024`.
- Cargo.lock may contain only the corresponding RAH workspace package-version
  changes.
- External dependency version/source/feature drift: none.

## 3. v0.15 scope

v0.15 is the bounded one-leaf repository directory-creation milestone.

- Public Tool: `repo.create-directory`.
- Authority: `RepositoryDirectoryCreationPolicy`.
- ADR: 0019,
  `docs/adr/0019-bounded-repository-directory-creation-authority.md`.

## 4. Contract

The narrow v1 contract is exactly one ordinary directory leaf at an explicit
repository-relative path:

- parent already exists;
- destination is absent;
- no recursion or `mkdir -p` semantics;
- no placeholder or implicit file creation;
- no Git mutation, staging, or commit;
- one possible-effect native attempt only;
- no retry/replay and no rollback/compensation.

Verified success is `status=directory_created_verified` and `uncertain=false`.
Git does not track empty directories, so clean `git status --short` is not
proof that no filesystem mutation occurred; the filesystem postcondition is
the creation proof.

## 5. Deterministic validation

Task 188 exact commands and results:

- `cargo fmt --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace` — PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `git diff --check` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS; 12 packages, all
  `0.15.0`, edition `2024`.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `cargo build -p rah-desktop --release` — PASS.

## 6. Windows live certification

Task 186 is the authoritative certification; Task 188 does not rerun it.

- Task 186 commit: `86c3eb38ddb3bf378c0001a11e594aba7cdea22d`.
- Exact-head CI: run `33827552359` — PASS.
- Status: Windows `repo.create-directory` live certification PASS.
- Evidence: `docs/plans/2026-09-04-windows-live-repo-create-directory-validation.md`.
- Certified baseline: `codex-cli 0.149.0`.
- `codex.exe` SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- `codex-code-mode-host.exe` SHA-256:
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.

## 7. Milestone audit

- Task 187 full SHA:
  `5e6ebd8b84d61b5018388c06df908553c5d77f33`.
- Exact-head CI: run `33828160578` — PASS.
- Status: PASS — ready for v0.15 release preparation.
- Release blockers: None.

## 8. Platform evidence

- Windows: live-certified.
- Ubuntu/Linux: deterministic validation where CI/tests establish it.
- Linux live certification: not established.

## 9. Known limitations / non-goals

No `mkdir -p`, recursive or multiple-directory creation, directory deletion or
rename/move/copy, overwrite/replacement, symlink or junction/reparse creation,
placeholder or `.gitkeep` creation, implicit file creation, arbitrary mode/ACL
authority, generic `fs.mkdir`, generic filesystem mutation, shell/process
mkdir, automatic staging/commit, rollback/compensation, or retry/replay after
possible effect. Linux live certification is not established.

## 10. Completed release checklist

- [x] Release candidate prepared.
- [x] Exact-head CI passed: Task 188, run `33829088735`.
- [x] Task 186 Windows live evidence reviewed and preserved.
- [x] Task 187 milestone audit confirmed.
- [x] Annotated `v0.15.0` tag created (Task 189).
- [x] Tag pushed (Task 189).
- [x] Immutable tag target verified (Task 189).
- [x] GitHub Release published (Task 189).
- [x] Release publication verified (Task 189).

Task 188 prepared the release candidate. Task 189 created and pushed the
annotated tag and published the GitHub Release. Task 190 records the completed
post-release state; it does not move the tag or republish the release.
