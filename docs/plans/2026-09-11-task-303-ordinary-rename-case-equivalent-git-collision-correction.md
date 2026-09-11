# Task 303 — Ordinary Rename Case-Equivalent Git Collision Correction

## Purpose

Correct the material ordinary `repo.rename-file` ADR 0018 gap identified by
Task 302. This is a focused security correction task, not v0.25 release
preparation.

## Defect

On Windows, `destination_git_absent` currently checks the requested Git path
against HEAD and the index using exact path bytes. Git path lookup is
case-sensitive while the supported Windows filesystem path identity is
case-insensitive. A tracked path can therefore be absent from the worktree,
remain in HEAD/index, and still collide with a differently cased requested
destination without the Git proof rejecting it.

## Scope

- Update ordinary ADR 0018 destination Git collision proof to compare relevant
  HEAD and index paths using the existing supported Windows path-equivalence
  rules.
- Preserve repository, path, source, destination, parent, reparse, nested
  repository, mount/volume, alias, and one-native-attempt constraints.
- Add deterministic coverage for a tracked-but-missing case-equivalent
  destination, including no native attempt and no effect.
- Revalidate the corrected proof and keep known-no-effect and uncertain
  classifications independent and conservative.
- Update only directly affected security/test documentation if needed.

## Out of scope

Do not change the reviewed HostExplicit route, ADR 0021 coordination, ADR 0026
review semantics, frontend/Tauri authority, public ordinary input schema,
workspace version, provider protocol, release tag, or GitHub Release.

## Acceptance evidence

The focused deterministic suite must prove that a tracked case-equivalent Git
collision is rejected before any native rename attempt. Workspace validation,
security documentation consistency, and exact-head CI must pass before a new
milestone audit is attempted.

## Implementation evidence

Completed on the Task 303 candidate. `RepositoryFileRenamePolicy::destination_git_absent`
now checks HEAD and the index independently. On Windows, index candidates use
Git's bounded `--icase-pathspecs` query. Because `git ls-tree` does not support
that option, HEAD uses a bounded component-by-component `ls-tree -z HEAD --`
walk: only the current tree level is enumerated, the actual returned Git tree
spelling is followed for the next component, and no recursive repository-wide
enumeration is used. On non-Windows platforms the existing literal,
case-sensitive query remains in place.

Every returned tree or index record is parsed strictly, including NUL framing,
record fields, valid Git modes/object IDs/stages, UTF-8 repository-relative
path representation, and normal path components. The actual candidate path is
then compared with the requested destination using the existing
`paths_equivalent` component-wise host rule; descendant candidates preserve the
previous tree/path collision behavior. Git process timeout, overflow, failed
observation, malformed records, unsupported encoding, and ambiguous tree
spelling fail closed. No Tool schema, status, permission, native primitive,
reviewed route, frontend/Tauri authority, provider route, or workspace version
changed.

Regression coverage uses real Git fixtures. The Windows-only correction tests
cover a tracked-but-missing `README.md` versus requested `readme.md`, an
index-only staged collision, intent-to-add, and non-stage-0 conflict entries;
each returns `precondition_failed`, records zero native attempts, preserves
the source, and leaves the destination absent. A case-sensitive non-Windows
fixture confirms distinct spellings remain admissible. Strict malformed
candidate framing/record tests fail closed.

## Validation evidence

- reviewed rename preparation: 24 passed;
- ordinary rename policy: 41 passed, serial, including the critical Windows
  tracked-but-missing case-equivalent fixture with native attempts `0`;
- Desktop deterministic suite: 219 passed, 10 ignored host-only tests;
- frontend syntax/authority tests and Tauri rename permission test: passed;
- Desktop release build: passed;
- `cargo fmt --check`: passed;
- `cargo check --workspace`: passed;
- `cargo test --workspace`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `git diff --check`: passed;
- `cargo metadata --no-deps --format-version 1`: passed; 13 packages, edition
  2024, workspace version 0.24.0.

The exact-head Windows live gate remains to be run after the final local
commit. Task 301's historical live result is not reused for this corrected
production candidate. Release readiness remains pending a fresh Task 304
milestone re-audit; no v0.25.0 tag or GitHub Release is authorized or created
by Task 303.
