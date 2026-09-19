# Task 368 - `repo.search` independent deterministic/security/currentness audit

Date: 2026-09-19
Starting HEAD: `7422c3271e11c31365102688f72cc2844c726f29`
Starting commit: `7422c32 docs: define repository discovery implementation contract`

## Scope and implementation state

Task 367 was present only as an uncommitted working-tree change. It was not
discarded, reset, overwritten, staged, or committed during this audit. The
frozen contract audited was:

`docs/plans/2026-09-19-task-366-bounded-repository-discovery-implementation-contract.md`

The exact Task 367 production files audited were:

- `crates/rah-tools/src/repository_search.rs`
- `crates/rah-tools/src/repository_observer.rs`
- `crates/rah-tools/src/repository_boundary.rs`
- `crates/rah-tools/src/trusted_profile.rs`
- `crates/rah-tools/src/lib.rs`
- `crates/rah-profile-composition/src/lib.rs`
- `crates/rah-desktop/src/effective_authority.rs`
- `crates/rah-desktop/src/host_invocation.rs`

No other pre-existing project file was modified. Task 368 made one narrow
hardening in the existing `repository_search.rs` file: text-mode omission
output now preserves the contract's `non_regular` category instead of folding
non-regular candidates into `changed_or_missing`. One regression test for that
output category and one linked-worktree path-mode assertion were added in the
same file. The audit document is the only additional path.

## Authority verdict

Final classification: **PASS WITH NARROW HARDENING**.

The implementation remains the frozen narrow extension:

```text
EffectClass       = ReadOnly
AuthorityCategory = RepositoryObservation
repository_bound  = true
PermissionLevel   = Execute
```

There is no new authority category, permission level, selector, persistence,
index, network capability, generic Git surface, external search executable,
HostExplicit kind, or dependency.

## Request and inventory audit

`SearchRequest::parse` performs host-side validation after serialized-size
checking. It rejects unknown fields and missing fields, accepts only `path` or
`text`, requires a non-empty query of at most 256 UTF-8 bytes, and rejects NUL,
CR, and LF. `path_prefix` is optional and is checked at 1024 UTF-8 bytes,
relative slash-separated syntax, non-empty components, no dot components, no
backslash, colon, NUL, or ASCII-case-insensitive `.git` component. JSON schema
limits are not relied on as security boundaries.

No model field is used as a repository selector, worktree selector, root,
executable, cwd, environment, Git argument, revision, pathspec, or limit.

The only inventory command is the host-fixed equivalent of:

```text
git --no-pager ls-files --cached --deduplicate -z --full-name --
```

It uses the canonical host-selected native executable, selected repository cwd,
the existing isolated observer environment, exact `safe.directory`,
`GIT_OPTIONAL_LOCKS=0`, and no query or prefix in argv. No shell, `git grep`,
filesystem walker, or external search process is used. Inventory stdout is
bounded to 4 MiB, records to 100,000, and the operation to 15 seconds.
Malformed NUL records, overflow, nonzero exit, timeout, or record overflow
return a sanitized error without partial matches.

## Search semantics and deterministic bounds

Inventory records are UTF-8 decoded without lossy replacement, validated as
bounded logical repository paths, sorted by UTF-8 path bytes, defensively
deduplicated, and prefix-filtered before matching. Invalid UTF-8 names are
omitted. Filesystem enumeration order, hash iteration, and Git incidental
ordering are not observable.

Path mode is literal, case-sensitive substring matching. Text mode reads the
current selected worktree bytes only and performs literal, case-sensitive,
single-line matching with 1-based LF line boundaries and CRLF handling. It
returns paths and line numbers only; it returns no snippets. `path_prefix` is a
literal subtree test and cannot make `src/foo` match `src/foobar`.

The reviewed bounds are:

| Item | Bound and behavior |
| --- | --- |
| request | 4 KiB serialized |
| logical path | 1024 UTF-8 bytes |
| inventory | 4 MiB stdout, 100,000 records |
| text file | 1 MiB |
| aggregate text reads | 16 MiB |
| path results | 128 |
| text result files | 64 |
| matching lines | 8 per file, 128 total |
| normalized output | 128 KiB |
| total operation | 15 seconds |

Evaluation saturation is successful partial output with `complete=false` and
only `result_limit` or `scan_byte_limit`. Structural/process failures return no
match list. The accounting order prevents beginning a file beyond the
aggregate byte bound; read bytes, including binary or invalid-text reads, are
accounted for. Non-regular candidates are now reported in `non_regular` for
both modes.

The focused production search target passed 7 tests after hardening. Those
tests cover closed request parsing, malformed and duplicate NUL inventory,
literal/case-sensitive path matching and result truncation, current worktree
text versus old content, tracked-only exclusion, prefix narrowing, CRLF/LF
line handling, nested-boundary rejection, linked-worktree isolation, and
before/after repository state preservation.

## Tracked-file, boundary, and link/reparse audit

The inventory is index-tracked only. Deleted tracked entries are omitted when
the selected worktree file is absent; staged-new and tracked-modified files are
eligible; untracked and ignored-untracked files are absent; tracked files that
match ignore patterns remain inventory candidates. Sparse-omitted files have
no eligible selected-worktree file and are omitted. There is no filesystem walk
fallback.

The existing `RepositoryNestedBoundaryPolicy` remains authoritative. Whole
repository inventory calls validate the complete observable tree and reject a
nested `.git` directory or file rather than skipping it. Windows matching is
ASCII-case-insensitive for `.git`; Unix keeps exact-case semantics. The change
to `repository_boundary.rs` only made the existing reparse predicate
`pub(crate)` for the search module; it did not broaden older capability
traversal.

Final candidates use `symlink_metadata`, boundary validation, and ordinary-file
checks. Symlink targets, directory symlinks, junctions, Windows reparse
directories, ambiguous reparse objects, and non-regular files are not read or
traversed. Text reads use the new final-entry no-follow open helper and verify
the opened file's type and length before and after the bounded read. The
existing best-effort TOCTOU limit remains explicit.

## Currentness and linked-worktree evidence

The operation acquires the existing canonical-root repository lease, revalidates
repository and Git-layout identity, validates the complete boundary, runs the
fixed inventory, revalidates, checks each candidate boundary, performs bounded
current-worktree scanning, performs final repository revalidation, and only
then serializes output. There is no retry or replay. Root, `.git` relationship,
linked-worktree registration, private/common Git identity, and Git executable
replacement are retained as fail-closed currentness conditions at the existing
observer boundaries. Ordinary file disappearance or type change is represented
by the documented best-effort omission path where safe.

The real linked-worktree fixture used main plus linked A and linked B with
unique tracked sentinels. With linked A selected, text search returned only
`a-only.txt` and line 1, and path search returned only `a-only.txt`; main and B
sentinels were absent in both modes. The result contained no private gitdir,
common gitdir, registration, backlink, commondir, absolute root, or filesystem
identity. The same test captured all three HEADs and statuses before and after
and observed no change.

## Trusted Profile, composition, authority, and HostExplicit

`repo.search` reuses the closed observer profile shape exactly:

```json
{"name":"repo.search","enabled":true,"permission":"execute","executable":"git","repository":"workspace"}
```

It requires explicit configuration, profile version remains 1, and missing
executable/repository, non-execute permission, unknown resources, workspace,
max-bytes, cwd-resource, and identity fields fail closed. Static loading does
not construct tools or execute Git. Effective composition constructs one
`RepositorySearchTool`, registers the canonical name, marks only the explicitly
validated first-party capability as registered, and retains duplicate-name
failure behavior. External MCP or Process Plugin metadata cannot masquerade as
the first-party repository observer or grant repository authority.

Effective Authority classifies the exact name as ReadOnly /
RepositoryObservation / repository-bound. It does not add a category or use a
name-prefix rule. Effective inventory remains redacted.

The HostExplicit set remains exactly these 11 names:

```text
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
```

`host_kind("repo.search") == None`, `repo.commit` and
`repo.create-directory` remain ineligible, and no `RepoSearch` invocation kind
was added.

## Privacy and no-intentional-mutation result

Search output is limited to bounded logical UTF-8 paths, line numbers, bounded
aggregate omission counters, mode, consistency, and completion/truncation
state. No query reflection, source snippet, absolute path, executable, cwd,
environment, private/common Git path, member ID, generation, raw stderr, or
filesystem identity is returned. Errors are sanitized. No search query or
content is persisted and no cache or hidden index exists.

Representative search tests captured HEAD, status, and index bytes before and
after; the linked fixture captured main/A/B HEAD and status before and after.
All were unchanged. The precise claim is **no intentional repository
mutation**.

## Parallel contention investigation

The required parallel signal was not treated as harmless without comparison.

- `cargo test --workspace` on the Task 367 tree exposed the existing
  contention-sensitive Desktop failures and stalled in a long-running lifecycle
  test; it was stopped after the failure pattern was established and therefore
  has no final overall count.
- The clean temporary baseline worktree at `7422c32` ran
  `cargo test -p rah-desktop` without Task 367 changes: 292 passed, 21 failed,
  and 16 ignored. The exact baseline failures were the three
  `provider_composition` fixture/build tests; `authorize_then_refresh...`,
  `binary_staged_review...`, `desktop_registry_commit...`, the four
  `deletion_hostexplicit...` tests, `observed_single_target_stage_and_unstage...`,
  `old_repository_selector...`, `desktop_successful_rename...`,
  `desktop_verified_directory_creation...`,
  `disconnect_revokes_pending_authorization...`,
  `repository_snapshot_matrix...`, the two Task 320/321 real-stage admission
  or publication tests, `task_321_e_stage_reservation_wins_over_activation`,
  `task_321_e_unstage_reservation_wins_over_activation`, and
  `task_320_stale_target_after_final_preparation...`.
- An isolated current `cargo test -p rah-desktop` run reproduced the same
  family: 307 passed, 7 failed, and 16 ignored. An isolated current
  `cargo test -p rah-tools` run had 308 passed and 18 failures, concentrated in
  existing repository-commit/file-info observer contention; all 7
  `repository_search` tests passed in that run.
- The Task 367 diff adds no static mutable fixture, shared temp name, inherited
  environment, global repository lock, or process collision. Search fixtures
  use process-and-counter-scoped names and the production search lease is the
  existing canonical-root lease. The clean-baseline reproduction proves the
  contention is pre-existing. Unrelated tests were not modified to mask it.

The known parallel contention is recorded as a validation limitation and does
not change the search audit verdict.

## Validation matrix

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS; only Git LF/CRLF warnings |
| `cargo test --workspace` | FAIL/stalled under known parallel contention; no search failure |
| `cargo test --workspace -- --test-threads=1` | PASS; serial workspace run completed with no failures before the one formatter-only hardening |
| `cargo test -p rah-tools repository_search -- --test-threads=1` | PASS, 7 passed |
| `cargo test -p rah-tools` | Parallel contention: 308 passed, 18 failed; all 7 search tests passed |
| `cargo test -p rah-profile-composition` | PASS, 4 passed |
| `cargo test -p rah-desktop` | Parallel contention: 307 passed, 7 failed, 16 ignored |
| `cargo metadata --no-deps --format-version 1` | PASS; 13 packages, all `0.30.0`, edition 2024 |

The serial workspace run before the narrow omission-category hardening reported 314
Desktop tests passed, 325 `rah-tools` tests passed, and all other workspace
targets passed; the post-hardening search target added and passed the seventh
search test. Formatting, check, clippy, focused tests, profile composition, and
metadata were rerun after hardening.

## Explicit nonclaims

This audit does not claim zero filesystem writes, rollback, replay safety,
global external-Git serialization, race-free TOCTOU, an OS sandbox, network
isolation, or a transactional snapshot. `consistency=best_effort` remains the
contract. It does not claim Windows live certification, Generic Tool Bridge
model-path certification, or Task 369 evidence. It does not start Task 369,
modify unrelated contention-sensitive tests, commit, push, tag, publish, or
create a release artifact.

## Final disposition

The narrow `non_regular` omission-category hardening is complete and preserves
Task 366 semantics. No contract or authority blocker was found. The working
tree is ready for a single production checkpoint commit containing the Task
367 implementation, this Task 368 audit document, and the narrow hardening;
the commit itself remains intentionally unperformed.
