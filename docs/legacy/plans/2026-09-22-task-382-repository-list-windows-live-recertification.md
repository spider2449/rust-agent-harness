# Task 382 — repo.list Windows Live Recertification

## Verdict

PASS WITH EXPLICIT LIVE NONCLAIMS

The corrected repo.list production tree passed fresh Windows live recertification. No production defect was found and no production Rust was changed by Task 382.

Explicit nonclaims:

- Adopted Codex baseline codex-cli 0.149.0 was unavailable. Ambient codex-cli 0.155.1 was not substituted, so real model-selected repo.list inference was not certified.
- Windows symlink creation was unavailable in the current configuration. Windows symlink live behavior is not certified; junction/reparse behavior was certified independently.

## Checkpoint and lineage

Starting committed checkpoint:

    ca96d1e5252bd4f320014981c562b6af27df71cf
    feat: add bounded repository structure listing

Preserved lineage:

    Task 379   FAIL — LIVE CERTIFICATION BLOCKER
    Task 380   PASS — PRODUCTION DEFECT CORRECTED
    Task 381   FAIL — CORRECTION WEAKENS BOUNDARY / REMAINS INCORRECT
    Task 381-A PASS — NARROW BOUNDARY HARDENING COMPLETE
    Task 381-B PASS — TASK 381-A BOUNDARY HARDENING VERIFIED
    Task 382   PASS WITH EXPLICIT LIVE NONCLAIMS

Task 379 and Task 381 were not rewritten.

## Fresh Windows environment

Captured again:

    Windows: Microsoft Windows 11 IoT Enterprise LTSC
    Version: 10.0.26100
    Build: 26100
    Architecture: x64
    rustc: 1.98.1 (48a229cea 2026-09-01)
    cargo: 1.98.1 (797e8a9bc 2026-08-05)
    Git: 2.55.0.windows.5

cargo metadata --no-deps --format-version 1 reported 13 packages, all
0.31.0, edition 2024, with no dependency drift.

Codex discovery found npm shims and codex-cli 0.155.1. Required 0.149.0
was unavailable and was not used.

## Fresh disposable fixture

The live test created a new native Git repository outside the RAH checkout,
with fresh main, linked A, and linked B worktrees. It did not reuse Task 379
or old sparse state. A contained the requested Cargo/crate/docs tree,
worktree-specific sentinels, tracked/modified/staged/ignored/untracked cases,
deleted tracked paths, deep sparse omissions, ordering inputs, and 130-child
saturation input.

The test captured HEAD, branch identity, porcelain-v2 status, and semantic
cached-diff evidence for main/A/B before observation and compared it after
observation. Fixture setup and sparse restoration were distinct from Tool
effects. No intentional repository mutation was observed.

## Production registry and authority

The real Desktop active-repository registry contained repo.list. This was
verified through admission, activation, desktop_tool_registry, and effective
composition, not only profile or unit fixtures.

The live effective entry was:

    EffectClass: ReadOnly
    AuthorityCategory: RepositoryObservation
    PermissionLevel: Execute
    repository_bound: true

It remained bound to the selected active worktree.

The exact HostExplicit set remained 11:

    fs.read
    repo.file-info
    repo.status
    repo.diff
    repo.diff-staged
    repo.create-branch
    repo.patch
    repo.edit-files
    repo.create-file
    repo.delete-file
    repo.rename-file

host_kind(repo.list), host_kind(repo.search), and
host_kind(repo.create-directory) were all None. No RepoList host kind, new
authority category, new permission, dependency, or version was introduced.
The Trusted Profile composition suite passed, including its test-fixture
prerequisite path; no missing fixture-binary failure recurred.

## Live listing matrix

Fresh A root request {} returned only direct children, including Cargo.toml,
a-only.txt, crates, docs, the staged-new file, and the tracked ignore-match
file. It did not disclose descendants, untracked or ignored-only directories,
physical empty directories, deleted-only parents, or Git metadata.

Deep navigation passed:

    {}                         -> Cargo.toml, a-only.txt, crates, docs, ...
    {"path":"crates"}           -> crates/alpha, crates/beta
    {"path":"crates/alpha"}    -> crates/alpha/src
    {"path":"crates/alpha/src"}-> crates/alpha/src/lib.rs

Empty path, ., .., /, trailing slash, backslash alias, .git, .GIT,
nonexistent target, file target, and extra fields depth, recursive, and
repository were rejected with sanitized errors. Root was represented only by
{}.

Modified tracked content remained visible. A staged-new tracked file was
visible. An already-tracked file matching .gitignore remained visible.
Untracked-only and ignored-untracked directories were absent. A tracked file
whose parent directory was removed was omitted without false boundary
rejection.

Directory synthesis appeared once per eligible tracked descendant. Physical
empty and physical-untracked directories were absent. Mixed ASCII,
case-different, Unicode, file, and directory entries were deterministic and
sorted by repository-relative UTF-8 byte order. More than 128 direct children
returned complete=false, truncation_reason=result_limit, exactly 128 entries,
and the same bounded prefix on repeat.

## Boundary and containment gates

The first-existing-object invariant was independently reviewed and covered by
focused boundary tests:

    NotFound              -> ascend exactly one lexical parent
    first existing object -> classify immediately and terminate

Safe ordinary directories accepted missing descendants. Symlink, junction or
reparse, regular-file, nested-repository, unsupported, and non-NotFound
conditions failed closed; no classification fell through to another ascent.
Root was the final valid authorization ancestor and no root-parent escape path
was available. Ordinary and linked-worktree .git metadata validated as
repository metadata and was not listed as content; .git and .GIT request
targets were rejected.

Fresh live junction/reparse fixtures pointed outside the repository at
external sentinels. repo.list and repo.search rejected the observation,
stopped at the unsafe existing object, and never exposed the sentinel. A
fresh nested repository and case-equivalent .GIT marker similarly caused
whole-observation rejection without partial results. Focused regular-file,
safe-directory, root-termination, and non-NotFound-error tests passed.

Live containment checks rejected lexical adversarial requests and kept all
observation inside active A. No outside repository, sibling worktree,
repository parent, external target, UNC/verbatim/drive-relative/ADS alias, or
backslash alias became an authorization ancestor.

## Sparse, deleted, currentness, and submodule behavior

The original sparse condition passed on the fresh fixture: a tracked candidate
with a missing immediate parent and a safe higher ancestor caused no false
boundary rejection. A multi-depth path equivalent to
sparse-only/deep/omitted/file.rs also remained omitted while root listing
succeeded.

The sparse unsafe-ancestor cross-check rejected the fresh junction case with
missing descendants. Focused symlink and reparse ancestor matrices passed;
missing depth did not change classification.

The final algorithm remains best-effort under missing-to-symlink,
directory-to-reparse, and directory-removal transitions. Later validation
remains conservative. No retry or replay guarantee was introduced.

The isolated submodule/Gitlink gate created a fresh local submodule after
ordinary listing checks. The repo.list observation failed closed at the
nested boundary; Gitlink contents were never traversed or disclosed, the
error was sanitized, and the fixture was cleaned before later gates. This is
the certified frozen submodule behavior, not a recursive listing.

Invalid arbitrary Git path bytes were not claimed on Windows; deterministic
invalid-UTF-8 tests remain the evidence for that platform limitation.

## Linked worktrees, switching, and bridge

With A active, root listing and search exposed A only. main-only.txt and
b-only.txt, private A/B gitdirs, common gitdir, worktree registration, and
object-store paths were not exposed. Host-owned activation to B produced
B-only output and removed A-only visibility. No Tool repository selector or
union registry was used.

The exact Trusted Profile capability shape remained enabled, execute, fixed
git, repository workspace, without wildcard admission or provider metadata
authority. The real Generic Tool Bridge dispatch tests for repo.list and
repo.search passed with no Tool-specific bridge special case.

## Privacy and mutation evidence

Live outputs, errors, authority serialization, and lifecycle surfaces exposed
only sanitized repository-relative data. They did not expose absolute roots,
nearest existing ancestors, external targets, private/common gitdirs, sibling
worktree roots, Git executable paths, filesystem IDs, RepositoryMemberId, or
raw stderr. External reparse and submodule sentinels were absent.

HEAD, branch/ref, porcelain-v2 status, and semantic cached-diff evidence were
unchanged by observations. Sparse setup/restoration can rewrite raw
skip-worktree index metadata; that fixture effect was not misclassified as a
repo.list or repo.search mutation. No intentional repository mutation was
observed. No claim of zero filesystem writes, transactional snapshot, global
Git lock, or rollback is made.

## Validation

Passed:

    cargo fmt --check
    cargo check --workspace
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    git diff --check
    cargo metadata --no-deps --format-version 1
    cargo test -p rah-tools -- --test-threads=1
    cargo test -p rah-profile-composition
    cargo test -p rah-desktop -- --test-threads=1
    cargo test -p rah-runtime-codex -- --test-threads=1
    cargo test --workspace -- --test-threads=1

The final isolated-submodule harness adjustment was additionally checked with
cargo fmt --check, cargo check -p rah-desktop, workspace clippy,
git diff --check, metadata, and the fresh live test; all passed. The full
workspace run had already passed immediately before that test-only relocation.

The historical timeout test
repository_diff_staged::unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty
passed in both package and workspace runs. It did not recur.

## Audit purity and files

Task 382 changed no production Rust. It made only audit-harness changes in
the existing test-only Windows certification module of
crates/rah-desktop/src/main.rs:

- fixed junction fixture staging by staging an ordinary path before replacing
  it with a junction;
- added staged-new, deleted-parent, multi-depth sparse, and isolated submodule
  live cases;
- made sparse-omitted state capture valid and compared semantic staged-diff
  evidence rather than sparse-mutated raw index bytes;
- restored fixture state after symlink, reparse, and sparse gates.

The only new Task 382 artifact is:

    docs/plans/2026-09-22-task-382-repository-list-windows-live-recertification.md

Pre-existing Task 380/381-A production files and Tasks 379–381-B documents were
preserved. No commit, push, tag, version bump, dependency change, or release
preparation occurred. HEAD remains the committed checkpoint above.

## Next task

Do not start automatically. Recommended next task:

    Task 383 — Corrected repo.list Production and Certification Checkpoint
