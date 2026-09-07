# Task 248 — Shared Non-Effectful `repo.patch` Preparation Foundation

## Status

Implementation complete; commit and exact-head CI remain.

## Checkpoint

- Repository root discovered with `git rev-parse --show-toplevel`.
- Required starting `HEAD` and `origin/master`:
  `c1ffde06ee954e75e53b8d110276b2f734178e12`.
- Task 247 CI required result: `34098085569 PASS`.
- Starting worktree was clean.
- Workspace target: 13 packages, version `0.20.0`, edition 2024.

## Shared-semantics refactor

`RepositoryPatchPreparer` uses the existing private repository worktree policy
for repository, target, Git, bounded-read, UTF-8/BOM, matching, uniqueness,
range, no-op, preimage, postimage, and file-size semantics. The mutation tool
continues to use the same `PatchRequest`, `capture_candidate`, and
`build_postimage` primitives. No second matcher or postimage builder is added.

## Public preparer API

`rah-tools` exposes a typed `RepositoryPatchPreparationRequest` containing only
`path`, `expected_old_text`, and `replacement_text`. The host-bound
`RepositoryPatchPreparer::new(git_executable, repository_root)` has only
`prepare(request)` as its public operation. It is not a `Tool`, has no generic
JSON preparation API, and exposes no mutation operation.

Successful preparation returns an opaque `RepositoryPatchPreparation` with the
exact canonical legacy `repo.patch` `ToolInput`, complete safe `R4` review,
review identity, preimage evidence, postimage evidence, and private target /
repository identity bindings for later ticketing.

## Zero-effect proof

Preparation acquires the existing repository lease, performs bounded Git
observations and target reads, computes the complete postimage in memory, and
does not call `Tool::execute`, `write_temporary`, or `replace_once`. Tests prove
the target, index, `HEAD`, refs, and temporary-artifact set remain unchanged and
replacement-attempt count remains zero.

## Review and bounds

The review is typed and serializable without private native identities. It
contains the repository-relative path, one half-open raw-file byte range with
BOM accounting, complete escaped old/replacement text, BOM state, pre/post EOF
state, intended effect/non-effects, unchanged-context omission, and pre/post
hashes and lengths. One shared deterministic escaping function marks CR, LF,
TAB, controls, format/invisible characters, and trailing spaces.

The existing request, text, aggregate, file/postimage, and replacement limits
remain unchanged. Preparation rejects oversized escaped changed material,
serialized review, or prepared representation with `ReviewTooLarge`; exact
changed content is never truncated. Equal old/replacement text returns typed
`NoEffect` without a preparation.

## Tests

Focused tests cover valid H1 preparation, canonical input and hashes, exact
review/range, one-execution semantic equivalence, zero effect, no-effect,
missing and multiple matches, dirty/staged/untracked states, malformed UTF-8,
BOM and newline behavior, controls/invisibles/trailing-space escaping,
BOM-aware offsets, postimage and review bounds, input bounds, and artifact
absence. Existing `repo.patch` tests remain green.

## Files

- `crates/rah-tools/src/repository_worktree_patch.rs`
- `crates/rah-tools/src/lib.rs`
- `docs/plans/2026-09-07-shared-repo-patch-preparation-foundation.md`

No Desktop, runtime-Codex, protocol, Cargo, lockfile, frontend, workflow, or
ADR files are in scope.

## Validation

Required sequential validation:

```text
cargo fmt --check
cargo check --workspace
cargo test -p rah-tools repository_worktree_patch -- --nocapture
cargo test -p rah-tools
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
git diff -- Cargo.toml Cargo.lock
```

Executed results:

- `cargo fmt --check`: passed.
- `cargo check --workspace`: passed.
- `cargo test -p rah-tools repository_worktree_patch -- --nocapture`: 29
  passed, 0 failed.
- `cargo test -p rah-tools`: 200 unit tests passed, with package integration
  suites also passing.
- `cargo test --workspace`: all workspace suites passed; no failures.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.
- `git diff --check`: passed.
- `cargo metadata --no-deps --format-version 1`: 13 packages, all `0.20.0`,
  edition 2024.
- `git diff -- Cargo.toml Cargo.lock`: empty.

## Commit

Preferred commit message: `feat: add non-effectful repository patch preparation`.

Commit: pending.

## Exact-head CI

Before push, run `git fetch origin` and require `origin/master` to remain
`c1ffde06ee954e75e53b8d110276b2f734178e12`. Push `master` normally and require
the exact Task 248 SHA CI run to be `master` / `push` / `completed` / `success`.

Exact-head CI: pending.

## Next task

Task 249 — Desktop HostExplicit `repo.patch` Backend Integration. Do not start
automatically.
