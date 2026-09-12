# Task 314 — Windows Case-Sensitive Nested-Marker and Stage/Unstage Snapshot Boundary Correction

Status: correction closed; final evidence recorded

## Authoritative checkpoint

- Current master: `336491926b5eeff0477eea4b0b0b15f073c0b367`
- Direct parent: `de8a9ed526814e19d800762fe1302094a22e0e0c`
- Task 313 exact-head CI: `34680412714` — PASS
- Task 313 verdict: Verdict B — CORRECTION STILL REQUIRED
- Task 313 changed scope: this Task 314 plan only
- Accepted ADR: ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary
- Current published release: RAH v0.25.0
- Immutable release source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`
- Annotated tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`
- GitHub Release: `387406579`
- Workspace baseline: 13 packages, version `0.25.0`, Rust edition 2024
- HostExplicit eligible set: exactly 11 names

## Purpose and findings

This is a focused repository-isolation correction. It does not implement
workspace membership, expand authority, change public Tool contracts, or make
`rah-sandbox::WorkspacePolicy` Git-aware.

Task 313 independently found two material gaps:

1. `RepositoryNestedBoundaryPolicy::validate_existing` probes only a literal
   `directory.join(".git")`, so a distinct `.GIT` entry in a Windows
   case-sensitive directory can evade target validation while the whole-tree
   observer rejects it.
2. Stage and Unstage construct a full `WorktreeSnapshot` that skips only an
   exact `.git` name and can read unrelated nested Repository-B files while
   proving a Repository-A index effect.

## Correction design

`rah-tools::repository_boundary` will own one shared marker-name predicate.
Windows will use deterministic ASCII-insensitive comparison for the exact
`.git` spelling; Unix-like systems will retain exact-case matching. No locale,
Unicode normalization, marker following, linked-worktree inference, or bare
repository heuristic is added. Immediate directory-entry enumeration will be
the authoritative `validate_existing` marker check and will fail closed on
enumeration errors or any matching marker, including file, symlink, reparse,
or multiple differently-cased markers.

`WorktreeSnapshot::capture` will use the same boundary semantics. Root metadata
will remain excluded, and each candidate child directory will be validated by
the repository boundary before recursion or any descendant file read. Ordinary
symlink handling remains non-following and unchanged. A nested Repository-B
marker therefore fails snapshot capture before `secret.txt` is opened, both
before the Stage effect and before the Unstage effect; currentness/revalidation
continues to check a boundary appearing after construction.

## Deterministic evidence

Add or extend tests for the shared matcher (`.git`, `.GIT`, `.Git`, mixed case
on Windows; exact `.git` on Unix), enumeration-based existing validation,
ordinary directories, marker directory/file/symlink or reparse forms, and the
existing observer symlink behavior. Add Stage and Unstage fixtures containing
`a.txt` plus unrelated `nested-b/.git/secret.txt`; instrument or otherwise
assert that snapshot logic does not read the secret, the index is unchanged,
and the mutation attempt count is zero. Retain target-boundary, appearing-after
construction, Commit, rename A→B/B→A/B→B, observer, and public-contract/
HostExplicit regressions.

A real per-directory case-sensitive Windows fixture will be additional evidence
only if it can run without privilege-dependent or unstable CI. Otherwise the
deterministic lower-level matcher/enumeration tests will document that live
case-sensitive NTFS behavior was not exercised.

## Public and authority audit

No Tool name, input/output schema, status enum, permission level, HostExplicit
allowlist, provider protocol, Trusted Profile schema, repository selector,
workspace membership, ADR, dependency edge, Cargo manifest, or `Cargo.lock`
change is authorized. Rename keeps its stronger capability-specific ancestry,
alias, collision, reparse, currentness, post-effect, and single-attempt proof.
Bare nested repositories, linked worktrees, race-free TOCTOU, rollback,
retry/replay, OS sandboxing, network isolation, and live Linux/macOS or
case-sensitive-NTFS certification remain nonclaims.

## Validation and delivery record

Required validation:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
```

Focused repository-boundary, Stage, Unstage, Commit, observer, fs.read,
file-info, patch, multi-edit, create-file, delete-file, rename, and
create-directory tests were run sequentially and passed:

- boundary filter: 13;
- Stage/Unstage internal policy tests: 9;
- Stage and Unstage integration tests: 6 each;
- Commit: 15;
- observer: 2;
- `fs.read`: 8;
- file-info: 4;
- patch: 49;
- multi-edit/preflight: 38;
- create-file: 11;
- delete-file: 11;
- rename policy: 42 and rename preparation: 24;
- create-directory: 6; and
- real nested-boundary integration: 1.

The final required gates all passed: `cargo fmt --check`,
`cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`git diff --check`, and `cargo metadata --no-deps --format-version 1`.
The final workspace test run passed 284 `rah-tools` unit tests and all
workspace integration suites with zero failures; existing host-only ignored
tests remained ignored. Metadata remained 13 packages at version `0.25.0`
and edition 2024. No Cargo manifest, dependency, or `Cargo.lock` drift was
observed.

The exact changed-file scope before delivery was:

- `crates/rah-tools/src/repository_boundary.rs`;
- `crates/rah-tools/src/git_stage.rs`;
- `crates/rah-tools/src/repository_rename_file.rs`; and
- this plan file.

The shared matcher is used by existing-target validation, whole-tree
observation, and Stage/Unstage snapshot traversal. Direct constructed `.git`
probes remain only for Repository-A root metadata identity/form checks; no
nested boundary decision relies on one. The Windows evidence is deterministic
enumerated-name and `.GIT` traversal coverage. A real per-directory
case-sensitive NTFS fixture was not run; no live case-sensitive NTFS
certification is claimed.

The Stage and Unstage unrelated-B regressions record the snapshot file-read
paths, assert `nested-b/secret.txt` is absent, compare the complete index before
and after, and assert the per-policy mutation-attempt counter is zero. Both
pre-existing and post-construction boundary cases reject before mutation.
Existing observer ordinary-symlink, Commit stale-boundary, and rename
cross-direction/stronger-proof regressions pass. The exact 11 HostExplicit
allowlist and public Tool/schema comparisons remain unchanged. Workspace
membership remains unimplemented.

Production commit:

- `1756b3a1f5848280017914169c648fb0efce76dc` —
  `fix: close remaining repository nested boundaries`.

There is no separate test-only commit. This plan was included with the
production correction; the final documentation evidence will be a separate
docs-only commit. Exact-head CI `34682248814` passed for the production head.
The final docs head will be pushed and receive its own exact-head CI result.

## Closure gate

The final report will state exactly one of:

- Outcome A — CORRECTION CLOSED
- Outcome B — BLOCKED

Outcome A requires both Task 313 material findings to be closed. The next task
is Task 315 — Repository Nested-Boundary Final Independent Re-Audit; workspace
membership remains out of scope.
