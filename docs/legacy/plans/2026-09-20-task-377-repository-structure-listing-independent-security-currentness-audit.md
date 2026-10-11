# Task 377 — Repository Structure Listing Independent Security / Currentness Audit

Date: 2026-09-20
Final verdict: PASS WITH NARROW HARDENING

## Scope and starting state

This independent audit reviews the intentionally uncommitted Task 376 production
changes against the frozen Task 375 contract.

Starting committed state:

- HEAD: 093d156d0f4cb3bc57ba0d13dcc0bc4b2d9ae1b1
- parent roadmap: 47f5de5
- origin/master: 2f9bd83957a7cefbd83e2ca377fd2125b8376616

The exact initial Task 376 file set was:

- crates/rah-desktop/src/effective_authority.rs
- crates/rah-desktop/src/host_invocation.rs
- crates/rah-desktop/src/main.rs
- crates/rah-profile-composition/src/lib.rs
- crates/rah-runtime-codex/src/bridge_tests.rs
- crates/rah-tools/src/lib.rs
- crates/rah-tools/src/repository_list.rs
- crates/rah-tools/src/repository_observer.rs
- crates/rah-tools/src/repository_search.rs
- crates/rah-tools/src/trusted_profile.rs
- docs/plans/2026-09-20-task-376-bounded-repository-structure-listing-implementation.md

All 11 paths were reviewed. No unrelated edit was found.

## Finding and narrow hardening

Task 376 conformed to the frozen schema, authority, fixed-inventory,
current-worktree, projection, and registration contracts except for one
adversarial projection condition. Its first-entry projection could classify a
logical child by input order if malformed inventory supplied both a file a and
a descendant a/b.

Narrow hardening was applied in crates/rah-tools/src/repository_list.rs:
identical duplicate kinds remain accepted, but a file/directory kind conflict
now returns a sanitized ToolError and no successful entry list. A deterministic
unit test covers this case. This is private implementation hardening only:
there is no new authority, request/output field, dependency, or public generic
inventory API.

## Contract, projection, and inventory

repo.list has exactly name repo.list and permission Execute. The closed request
accepts {} and a relative path such as crates/rah-tools/src; unknown fields and
the Task 375 invalid forms are rejected. The root has one representation:
omitted path, serialized as a null output path. No alternate root alias is
admitted.

The result contains direct children only. The required fixture projects the
root to README.md and crates, and crates to crates/rah-core and
crates/rah-tools; it does not return crates/rah-tools/src or its files for a
crates request.

Directories are synthesized only from eligible tracked descendants.
Untracked-only, ignored-untracked-only, deleted-tracked-only, and
sparse-omitted-only directories are absent. Duplicate descendants collapse to
one directory. Physical directory existence is not disclosure authority.
Conflicting file/directory candidates fail closed rather than using map
last-write-wins semantics.

The only Git inventory command remains exactly:

git --no-pager ls-files --cached --deduplicate -z --full-name --

No request data or model pathspec is appended. There is no shell, filesystem
walker, git ls-tree, or git status substitution. Extracted inventory/path/
candidate helpers are private/internal.

Eligibility is tracked + currently present + eligible regular file. Clean,
modified, staged-new present, and tracked ignore-match files are visible.
Deleted, sparse-omitted, untracked, and ignored-untracked files are absent.
Symlink/reparse and other non-regular candidates are omitted as specified; no
link is followed. Directory symlink, junction, reparse, ambiguous ancestry,
or observable nested .git fails the whole observation without partial entries.

Gitlinks/submodules are not traversed; their contents cannot enter the outer
listing. Invalid UTF-8 paths are omitted as non-addressable, without lossy
conversion or U+FFFD. Overlong candidate paths cannot synthesize a directory
or leak a partial path. Windows invalid-UTF-8 live behavior is not claimed.

Ordering is deterministic repository-relative UTF-8 byte order. Saturation is
status ok, complete false, truncation_reason result_limit, with a deterministic
bounded prefix. Request, path, inventory, record, candidate, entry, serialized
output, NUL-record, and total-time bounds are enforced. Structural failures
are sanitized and return no partial entries; paths are never silently shaved.

## Shared helper and repo.search regression

The shared extraction was reviewed line by line. repo.search request schema,
query and path_prefix validation, fixed inventory, currentness sequence,
bounds, omission categories, ordering, result schema, timeout accounting, and
linked-worktree behavior remain semantically unchanged. No public generic
inventory authority was introduced.

The current-tree rah-tools serial suite passed 332 tests. Its repository search
tests cover literal/case-sensitive path and text matching, prefix handling,
current-worktree bytes, CRLF line numbering, untracked and ignored exclusion,
tracked ignore-match inclusion, non-regular omission, bounds, linked-worktree
isolation, and malformed inventory. This is semantic regression evidence, not
a claim of byte-for-byte comparison with a separate baseline binary.

## Currentness, races, leases, and boundaries

The observer uses one total bounded timeout, subtracting elapsed time across
phases; it does not grant a fresh full timeout per phase. Identity, layout,
boundary, and candidate checks use the frozen revalidation sequence.
symlink_metadata is used for candidate classification. Lease ownership is RAII
scoped through success, errors, timeout, and cancellation. Shared lease
release tests passed; no leaked child, lease, or detached task was found.

The operation is best effort. It does not retry disappearance, replacement,
identity change, Git-layout change, or other TOCTOU races. Requested-directory
existence comes from the final eligible logical projection: a physical
untracked-only directory is not a browse target, an eligible tracked file is
not a directory, and a missing logical prefix fails sanitized.

Nested repository tests, including Windows case variants where supported, fail
closed before partial output. Root .git, private/common gitdirs, worktree
registration data, sibling roots, Git executable paths, reparse targets, and
filesystem identities are not output. Tool output, errors, bridge output,
effective authority, profile composition, and Desktop activity remain
sanitized to safe repository-relative paths.

Linked-worktree tests and selected-repository composition establish isolation:
only the selected worktree structure is observed; main/sibling sentinels and
private/common/registration data are absent. Switching composes a fresh
selected registry; there is no union or sibling selector.

Repository-list tests compare repository state around observation, and linked
observer tests cover selected-worktree isolation. No intentional mutation of
worktree, index, HEAD, or selected ref was found. This is not a claim of zero
filesystem writes, snapshot isolation, transactional observation, rollback, or
a global external-Git lock.

## Profile, registration, authority, and bridge

Trusted Profile admission is explicit and remains the closed profile family:
name repo.list, enabled true, permission execute, executable git, repository
workspace. No profile version, top-level field, depth/result-limit argument,
environment argument, or selector was added. Observer presence does not
implicitly admit repo.list and no repo.* wildcard inference exists.

Desktop active-repository production registry construction explicitly registers
repo.list, independently of profile, bridge, and authority tests. The
deterministic Desktop registry test passed. Effective Authority is exactly:
EffectClass ReadOnly, AuthorityCategory RepositoryObservation,
repository_bound true, PermissionLevel Execute.

HostExplicit remains exactly these 11 tools:
fs.read, repo.file-info, repo.status, repo.diff, repo.diff-staged,
repo.create-branch, repo.patch, repo.edit-files, repo.create-file,
repo.delete-file, repo.rename-file.
host_kind(repo.list) and host_kind(repo.search) are None, and
repo.create-directory remains ineligible. No enum or allowlist expansion
occurred.

The Generic Tool Bridge test follows the canonical definition through the
private dynamic alias, ToolRequested, registry dispatch, repo.list, and
ToolFinished. There is no repo.list-specific bridge special case. This is
deterministic bridge plumbing evidence, not real model-selected inference.

## Serial timeout investigation and baseline A/B

Task 376 reported this exact failure:

- crate: rah-tools
- test: repository_diff_staged::unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty
- failure: error: repository observation exceeded its total timeout

The source was the repository-observer ToolError, not a Rust harness timeout
or separately reported child-process timeout. Task 376 recorded no duration.
The exact test was run sequentially three times on the current tree before
hardening; all passed, with test durations about 5.40–5.45 seconds. It passed
again after hardening in 6.15 seconds.

A clean detached worktree at 093d156d0f4cb3bc57ba0d13dcc0bc4b2d9ae1b1 was
created externally, kept clean, and removed after evidence collection. There,
the exact test passed in 6.04 seconds, the serial rah-tools suite passed 326
tests, and the full serial workspace gate completed without failure.

After hardening, rah-tools passed 332 tests, all focused suites passed, and
the final current-tree full serial workspace gate exited 0. The historical
timeout did not reproduce in isolation or in the clean baseline and was not
uniquely implicated in Task 376. It is classified as non-reproduced transient
timeout. The original failing aggregate run remains recorded as a failure, not
relabeled as a pass.

## Validation and metadata

All required commands completed successfully:

- cargo fmt --check
- cargo check --workspace
- cargo clippy --workspace --all-targets --all-features -- -D warnings
- git diff --check
- cargo metadata --no-deps --format-version 1
- cargo test -p rah-tools -- --test-threads=1
- cargo test -p rah-profile-composition
- cargo test -p rah-desktop -- --test-threads=1
- cargo test -p rah-runtime-codex -- --test-threads=1
- cargo test --workspace -- --test-threads=1

Focused totals were: rah-tools 332 passed/0 failed; profile-composition
4 passed/0 failed; Desktop 315 passed/17 ignored/0 failed; runtime-codex
85 passed/1 ignored/0 failed. The final full serial workspace gate exited 0.
git diff --check exited 0, with only LF-to-CRLF working-copy warnings.

Metadata reported 13 packages, all version 0.31.0 and edition 2024. There was
no dependency drift, Cargo version bump, ADR addition, PermissionLevel change,
AuthorityCategory change, or HostExplicit change.

## Final state and nonclaims

The final changed-file set is the 11 Task 376 paths above plus this artifact:
docs/plans/2026-09-20-task-377-repository-structure-listing-independent-security-currentness-audit.md

HEAD remains 093d156d0f4cb3bc57ba0d13dcc0bc4b2d9ae1b1. The working tree contains
Task 376, the narrow projection hardening, and this audit record. No Task 377
commit was created and nothing was pushed. origin/master remains
2f9bd83957a7cefbd83e2ca377fd2125b8376616.

This audit does not claim snapshot isolation, race freedom, zero filesystem
writes, rollback, automatic retry, global external-Git locking, OS sandboxing,
network isolation, Windows invalid-UTF-8 behavior, all-platform parity, or
real model-selected/live Windows certification.

Recommended next task: Task 378 — Repository Structure Listing Production
Checkpoint.
