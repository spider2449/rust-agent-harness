# Task 183 — Core Repository Directory Creation Implementation

## Scope

Task 183 implements the narrow core authority accepted by ADR 0019. The
operation creates exactly one absent ordinary directory leaf beneath an
already-existing validated repository parent. Desktop, Trusted Profile, MCP,
Process Plugin, Generic Codex Bridge, live validation, release, and version
work are outside this task.

## Starting state

- Starting `HEAD`: `75ade9c399488326db967e745545ba5cd881be34`
- Starting `origin/master`: `75ade9c399488326db967e745545ba5cd881be34`
- Working tree: clean
- v0.14.0 release state remains immutable.
- ADR authority: `docs/adr/0019-bounded-repository-directory-creation-authority.md`

## Implementation

- Added the separate host-owned `RepositoryDirectoryCreationPolicy` and
  opaque `RepositoryDirectoryCreationAuthority`.
- Added the crate-private directory-object identity comparison needed to avoid
  treating normal Unix directory link-count changes as identity replacement.
- Added the public `repo.create-directory` Tool with the closed request
  schema `{"path":"existing-parent/new-directory"}`.
- Reused the shared repository mutation lease, logical path validation,
  reparse/link ancestry checks, and filesystem identity checks.
- Revalidated repository identity, parent identity, target absence, and direct
  Git metadata snapshots immediately before the native attempt.
- Added one-leaf native creation: `mkdirat` relative to a validated directory
  descriptor on Unix and the existing handle-relative `NtCreateFile` wrapper
  on Windows. No recursive or shell/process fallback is used.
- Added sanitized JSON result classification for `invalid_input`,
  `precondition_failed`, `directory_created_verified`, `known_no_effect`, and
  `uncertain`. Verified success includes `git_metadata_changed: false`.

## Tests and validation

Deterministic tests cover root and nested-parent success, empty-directory
postconditions, closed input and path rejection, missing parents, file
parents, existing targets, one-attempt race behavior, and uncertain-result
non-replay/non-compensation behavior. Native platform behavior remains gated
by the target platform; no Linux live-certification or Windows live claim is
made here.

Validation results:

- `cargo fmt --check` — passed.
- `cargo test -p rah-tools repository_create_directory --lib` — passed,
  5 focused tests.
- `cargo check --workspace` — passed.
- `cargo test --workspace` — passed; all executed tests passed, with only the
  repository's existing host-only ignored tests omitted.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed.
- `cargo metadata --no-deps --format-version 1` — passed; 12 packages, all
  version `0.14.0`, all edition 2024, with no dependency drift.
- `git diff --check` — passed.
- Exact-head CI run `33824623988` for commit
  `135b7c174cfdde33519ad6a4e574762641c0329d` — passed formatting, workspace
  check, workspace tests, and workspace lint.

The current Windows host executed the Windows-gated native helper tests. No
Linux live-certification or Desktop/live validation was performed.

## Preserved non-goals

No Desktop integration, profile schema/loading, provider integration, live
validation, version/dependency/release change, directory deletion or rename,
recursive creation, placeholder file, Git/index/commit mutation, retry,
replay, cleanup, or rollback was added.

## Next task

Task 184 should verify normal ToolRegistry composition and Generic Codex Tool
Bridge advertisement/request/result behavior without adding a private alias or
manufacturing host authority.
