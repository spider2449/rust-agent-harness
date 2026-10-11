# Task 185 — Desktop Repository Directory-Creation Integration

## Scope and baseline

Task 185 integrates the accepted `repo.create-directory` capability into the
Windows-gated Desktop host path and hardens its deterministic lifecycle
coverage. The starting `HEAD` was
`31a1243ded7ff420489ea08a8b4de9bad352590e`, equal to `origin/master`, with a
clean worktree. ADR 0019 and the Task 183 core and Task 184 composition plans
remain authoritative baselines.

No Windows live certification is performed in this task.

## Desktop composition

`choose_repository` now constructs a host-owned
`RepositoryDirectoryCreationAuthority` from the selected repository path.
`DesktopRepository` retains that opaque authority with the selected canonical
repository snapshot and validates that its bound root matches the snapshot.
`desktop_tool_registry` registers `repo.create-directory` only when that
explicit authority is present. The public Tool name and closed request schema
remain unchanged; the generic Codex Tool Bridge is not changed.

The authority is not global and is not derived from frontend state, profile
data, Execute permission, or other repository mutation authorities. Replacing
the selected repository replaces the whole `DesktopRepository` snapshot and
increments `repository_generation`. Existing connection publication checks,
stale-runtime rejection, reconnect-required state, and Send generation checks
remain the lifecycle boundary. A fresh connection composes the fresh selected
repository snapshot; no authority is reused across selection.

## Mutation lifecycle

`repo.create-directory` participates in the existing generic Desktop tool
activity refresh path. A verified `directory_created_verified` result is
treated as a repository mutation even when Git status, index, HEAD, and refs
remain unchanged. The existing refresh path therefore refreshes repository
presentation and invalidates outstanding reviewed-commit authorization without
staging a directory or fabricating Git changes.

Possible-effect uncertainty remains conservative: it is not retried, replayed,
deleted, or compensated. The generic finished-tool mutation boundary is the
place where stale review state is invalidated; known precondition failures are
not represented as verified mutations.

## Deterministic tests

Added Desktop coverage verifies:

- explicit directory authority adds the public Tool to the effective registry;
- authority absence does not infer directory creation from other tools;
- a real empty directory can be created through the Desktop registry with a
  verified result while `git status --porcelain=v1` remains clean;
- the public Tool name is used by Desktop activity lifecycle handling and
  triggers repository refresh classification; and
- verified directory creation invalidates an outstanding reviewed-commit
  authorization, including when Git has no visible status change.

Existing Task 175C lifecycle tests continue to cover selection while
Connecting, repository-generation changes, stale connection publication,
reconnect-required behavior, and stale turn rejection. Core uncertainty and
no-replay behavior remain covered by Task 183 tests.

## Files and boundaries

Changed production files:

- `crates/rah-desktop/src/main.rs`
- `crates/rah-tools/src/repository_create_directory.rs`

The tools change adds only a narrow root-binding matcher needed by Desktop to
validate that its selected repository snapshot and opaque authority agree.
No frontend files changed and no frontend command or Tauri permission was
introduced. Trusted Profile code and schema did not change and do not
manufacture directory authority. No dependency, version, release, or Cargo
metadata change was made. No `.vscode/` file changed.

No production Desktop path uses recursive creation, `create_dir_all`, Git
mutation, placeholder files, directory deletion, or global current-directory
state. No live certification was performed.

## Validation

Validation completed:

- `cargo fmt --check` — passed.
- `cargo test -p rah-tools --lib` — 148 passed.
- `cargo test -p rah-desktop` — 139 passed, 2 ignored.
- `cargo check --workspace` — passed.
- `cargo test --workspace` — passed; all executed tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.
- `git diff --check` — passed.
- `cargo metadata --no-deps --format-version 1` — 12 packages, all `0.14.0`,
  edition 2024; no dependency drift.
- `cargo build -p rah-desktop --release` — passed.

The focused create-directory and review-revocation tests were also run
individually and passed. `git grep -n "set_current_dir"` found only the
instruction/documentation references, not an implementation use.

The final report must also record exact Git state, package/version metadata,
the absence of dependency drift, and the exact-head CI result after push.

## Recommended next task

After Tasks 183–185 deterministic core, bridge, and Desktop coverage is
reviewed, recommend Task 186 based on evidence: use the hardening/fixture-audit
task only if material deterministic or platform gaps remain; otherwise proceed
to the separately scoped Windows live Codex validation task. Do not perform
that live gate here.
